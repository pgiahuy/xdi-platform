// SPDX-License-Identifier: Apache-2.0
/* engine/store/domains.rs - gói khái niệm ngành của một workspace (0014_domain_concepts.sql, #229): nạp khởi tạo từ các
gói nhúng trong binary (engine/domains.rs), đọc cho prompt và cho màn Khái niệm ngành, thêm và sửa khái niệm. Kiểm
khái niệm trước khi ghi và quyết định khi nào nạp khởi tạo là việc của service/domains/.

Nạp khởi tạo chỉ thêm gói và khái niệm của file mà workspace chưa có (so theo khóa), không bao giờ ghi đè: khái niệm
admin đã sửa hay đã tắt giữ nguyên, kể cả khi file đổi chữ ở bản sau. Đổi lại, sửa chữ một khái niệm có sẵn trong file
không tới được workspace đã nạp; chỉ khái niệm hay gói mới (khóa mới) tới được. */

use chrono::{DateTime, Utc};
use sqlx::types::Json;
use uuid::Uuid;

use super::{db_error, Store};
use crate::auth::WorkspaceId;
use crate::engine::domains::{Calculation, DomainConcept, DomainPackage, SanityCheck};
use crate::error::AppError;

/// Một gói trong workspace.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct DomainPackRecord {
    pub id: Uuid,
    /// Tên file gói không đuôi, cũng là giá trị của sources.domain (gói của một dataset).
    pub key: String,
    pub name: String,
    pub description: String,
}

/// Một khái niệm trong workspace, đủ trường cho màn quản trị.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DomainConceptRecord {
    pub id: Uuid,
    pub pack_id: Uuid,
    /// Id của khái niệm trong file gói; None với khái niệm admin thêm.
    pub key: Option<String>,
    pub name: String,
    pub description: String,
    pub synonyms: Vec<String>,
    pub calculations: Json<Vec<Calculation>>,
    pub dimensions: Vec<String>,
    pub sanity_checks: Json<Vec<SanityCheck>>,
    pub enabled: bool,
    /// None khi chưa ai sửa từ lúc nạp khởi tạo.
    pub updated_at: Option<DateTime<Utc>>,
}

/// Khái niệm admin gửi lên, đã kiểm (service/domains/check.rs).
#[derive(Debug, Clone)]
pub struct NewDomainConcept {
    pub name: String,
    pub description: String,
    pub synonyms: Vec<String>,
    pub calculations: Vec<Calculation>,
    pub dimensions: Vec<String>,
    pub sanity_checks: Vec<SanityCheck>,
    pub enabled: bool,
}

const CONCEPT_COLUMNS: &str =
    "id, pack_id, key, name, description, synonyms, calculations, dimensions, \
                               sanity_checks, enabled, updated_at";

impl Store {
    /// Cặp (khóa gói, khóa khái niệm) workspace đã có; gói chưa có khái niệm nào thì khóa khái niệm là None. Khái niệm
    /// admin thêm (key null) không có trong danh sách. service/domains/ so với các gói nhúng để biết có cần nạp không.
    pub async fn list_domain_seed_keys(
        &self,
        ws: &WorkspaceId,
    ) -> Result<Vec<(String, Option<String>)>, AppError> {
        sqlx::query_as(
            "SELECT p.key, c.key
             FROM domain_packs p
             LEFT JOIN domain_concepts c ON c.pack_id = p.id AND c.key IS NOT NULL
             WHERE p.workspace_id = $1",
        )
        .bind(ws.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list domain seed keys", e))
    }

    /// Thêm các gói và khái niệm của packages mà workspace chưa có, trong một giao dịch. Chạy lại không đổi gì (ON
    /// CONFLICT DO NOTHING); hai lần chạy cùng lúc thì lần sau chờ lần đầu xong rồi cũng không thêm gì.
    pub async fn seed_domain_packages(
        &self,
        ws: &WorkspaceId,
        packages: &[DomainPackage],
    ) -> Result<(), AppError> {
        let error = |e| db_error("seed domain packages", e);
        let mut tx = self.pool.begin().await.map_err(error)?;
        for package in packages {
            sqlx::query(
                "INSERT INTO domain_packs (workspace_id, key, name, description) VALUES ($1, $2, $3, $4)
                 ON CONFLICT (workspace_id, key) DO NOTHING",
            )
            .bind(ws.as_uuid())
            .bind(&package.id)
            .bind(&package.name)
            .bind(&package.description)
            .execute(&mut *tx)
            .await
            .map_err(error)?;
            let pack_id: Uuid = sqlx::query_scalar(
                "SELECT id FROM domain_packs WHERE workspace_id = $1 AND key = $2",
            )
            .bind(ws.as_uuid())
            .bind(&package.id)
            .fetch_one(&mut *tx)
            .await
            .map_err(error)?;
            for (position, concept) in package.concepts.iter().enumerate() {
                insert_seed_concept(&mut tx, ws, pack_id, position, concept)
                    .await
                    .map_err(error)?;
            }
        }
        tx.commit().await.map_err(error)
    }

    /// Mọi gói của workspace, theo khóa.
    pub async fn list_domain_packs(
        &self,
        ws: &WorkspaceId,
    ) -> Result<Vec<DomainPackRecord>, AppError> {
        sqlx::query_as(
            "SELECT id, key, name, description FROM domain_packs WHERE workspace_id = $1 ORDER BY key",
        )
        .bind(ws.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list domain packs", e))
    }

    pub async fn find_domain_pack(
        &self,
        ws: &WorkspaceId,
        key: &str,
    ) -> Result<Option<DomainPackRecord>, AppError> {
        sqlx::query_as(
            "SELECT id, key, name, description FROM domain_packs WHERE workspace_id = $1 AND key = $2",
        )
        .bind(ws.as_uuid())
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_error("find domain pack", e))
    }

    /// Mọi khái niệm của workspace, kể cả đang tắt, theo gói rồi theo thứ tự của file (khái niệm admin thêm đứng sau).
    pub async fn list_domain_concepts(
        &self,
        ws: &WorkspaceId,
    ) -> Result<Vec<DomainConceptRecord>, AppError> {
        let sql = format!(
            "SELECT {CONCEPT_COLUMNS} FROM domain_concepts WHERE workspace_id = $1
             ORDER BY pack_id, position, created_at"
        );
        sqlx::query_as(&sql)
            .bind(ws.as_uuid())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db_error("list domain concepts", e))
    }

    /// Gói có khóa này kèm các khái niệm đang bật, cùng thứ tự với file, cho prompt. None khi workspace không có gói
    /// này.
    pub async fn load_domain_package(
        &self,
        ws: &WorkspaceId,
        key: &str,
    ) -> Result<Option<DomainPackage>, AppError> {
        let Some(pack) = self.find_domain_pack(ws, key).await? else {
            return Ok(None);
        };
        let concepts = self
            .list_domain_concepts(ws)
            .await?
            .into_iter()
            .filter(|c| c.pack_id == pack.id && c.enabled)
            .map(build_concept)
            .collect();
        Ok(Some(DomainPackage {
            id: pack.key,
            name: pack.name,
            description: pack.description,
            concepts,
        }))
    }

    /// Thêm một khái niệm do admin viết vào cuối gói. Trả id của dòng mới.
    pub async fn add_domain_concept(
        &self,
        ws: &WorkspaceId,
        pack_id: Uuid,
        concept: &NewDomainConcept,
        created_by: Uuid,
    ) -> Result<Uuid, AppError> {
        // Vị trí tính trong cùng câu INSERT. updated_by ghi người thêm; updated_at để null vì khái niệm chưa bị sửa.
        sqlx::query_scalar(
            "INSERT INTO domain_concepts (workspace_id, pack_id, name, description, synonyms, calculations,
                                          dimensions, sanity_checks, enabled, position, updated_by)
             SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9, COALESCE(MAX(position), -1) + 1, $10
             FROM domain_concepts WHERE pack_id = $2
             RETURNING id",
        )
        .bind(ws.as_uuid())
        .bind(pack_id)
        .bind(&concept.name)
        .bind(&concept.description)
        .bind(&concept.synonyms)
        .bind(Json(&concept.calculations))
        .bind(&concept.dimensions)
        .bind(Json(&concept.sanity_checks))
        .bind(concept.enabled)
        .bind(created_by)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| db_error("add domain concept", e))
    }

    /// Ghi đè một khái niệm của gói. false khi không có khái niệm id này trong gói của workspace.
    pub async fn update_domain_concept(
        &self,
        ws: &WorkspaceId,
        pack_id: Uuid,
        id: Uuid,
        concept: &NewDomainConcept,
        updated_by: Uuid,
    ) -> Result<bool, AppError> {
        let result = sqlx::query(
            "UPDATE domain_concepts
             SET name = $4, description = $5, synonyms = $6, calculations = $7, dimensions = $8,
                 sanity_checks = $9, enabled = $10, updated_at = now(), updated_by = $11
             WHERE workspace_id = $1 AND pack_id = $2 AND id = $3",
        )
        .bind(ws.as_uuid())
        .bind(pack_id)
        .bind(id)
        .bind(&concept.name)
        .bind(&concept.description)
        .bind(&concept.synonyms)
        .bind(Json(&concept.calculations))
        .bind(&concept.dimensions)
        .bind(Json(&concept.sanity_checks))
        .bind(concept.enabled)
        .bind(updated_by)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("update domain concept", e))?;
        Ok(result.rows_affected() > 0)
    }
}

/// Một khái niệm của file; đã có khái niệm cùng khóa trong gói thì bỏ qua.
async fn insert_seed_concept(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ws: &WorkspaceId,
    pack_id: Uuid,
    position: usize,
    concept: &DomainConcept,
) -> Result<(), sqlx::Error> {
    // Gói có vài chục khái niệm là nhiều; vị trí vượt i32 không xảy ra, nhưng đổi kiểu có kiểm vẫn rẻ hơn ép kiểu.
    let position = i32::try_from(position).unwrap_or(i32::MAX);
    sqlx::query(
        "INSERT INTO domain_concepts (workspace_id, pack_id, key, name, description, synonyms, calculations,
                                      dimensions, sanity_checks, position)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT (pack_id, key) DO NOTHING",
    )
    .bind(ws.as_uuid())
    .bind(pack_id)
    .bind(&concept.id)
    .bind(&concept.name)
    .bind(&concept.description)
    .bind(&concept.synonyms)
    .bind(Json(&concept.calculations))
    .bind(&concept.dimensions)
    .bind(Json(&concept.sanity_checks))
    .bind(position)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Dòng của bảng thành khái niệm dạng file, cho prompt. Khái niệm admin thêm không có khóa: dùng id của dòng.
fn build_concept(record: DomainConceptRecord) -> DomainConcept {
    DomainConcept {
        id: record.key.unwrap_or_else(|| record.id.to_string()),
        name: record.name,
        description: record.description,
        synonyms: record.synonyms,
        calculations: record.calculations.0,
        dimensions: record.dimensions,
        sanity_checks: record.sanity_checks.0,
    }
}
