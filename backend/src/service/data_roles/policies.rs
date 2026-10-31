// SPDX-License-Identifier: Apache-2.0
/* service/data_roles/policies.rs - luật lọc dòng của một vai dữ liệu: kiểm, chuẩn hóa rồi thay cả bộ (set_policies), và
danh sách cột dùng được cho luật của một bảng (filter_columns) cho màn chọn bảng, cột, giá trị.

Luật chỉ đặt trên cột chiều (role dimension) của phiên bản hiện hành: cột chia nhóm như vùng, thị trường, có danh sách
giá trị trong hồ sơ (tới 200 giá trị) để admin chọn. Cột số đo, thời gian hay chữ tự do lọc theo danh sách giá trị không
có nghĩa. Giá trị không có trong danh sách vẫn nhận (dữ liệu có thể đổi ở lần chụp sau), kèm lời báo để admin kiểm
chính tả: gõ sai thì người mang vai không thấy dòng nào có giá trị đó.

Giá trị và luật được sắp xếp trước khi lưu, để store so được hai bộ luật bằng == (không cho hai vai trùng bộ luật,
#167). */

use std::collections::BTreeSet;

use uuid::Uuid;

use super::contract::{FilterColumnResponse, SetPoliciesRequest, SetPoliciesResponse};
use crate::auth::{Member, WorkspaceId};
use crate::engine::store::{PoliciesOutcome, ProfileColumn, ProfileRole, RolePolicy};
use crate::{error::AppError, state::AppState};

/// Bảng của một luật: tên để báo lỗi, cột của phiên bản hiện hành.
struct PolicyTable {
    name: String,
    columns: Vec<ProfileColumn>,
}

/// Thay toàn bộ luật của vai role_id. Trả bộ luật đã lưu và lời báo cho admin.
pub async fn set_policies(
    state: &AppState,
    member: &Member,
    role_id: Uuid,
    req: SetPoliciesRequest,
) -> Result<SetPoliciesResponse, AppError> {
    member.require_admin()?;
    let ws = &member.workspace;
    let mut policies = Vec::new();
    let mut warnings = Vec::new();
    let mut seen = BTreeSet::new();
    for policy in req.policies {
        let policy = build_clean_policy(policy);
        let table = load_policy_table(state, ws, policy.table_id).await?;
        if !seen.insert((policy.table_id, policy.column.clone())) {
            return Err(AppError::BadRequest(format!(
                "Bảng {} có hai luật trên cột {}; gộp giá trị vào một luật",
                table.name, policy.column
            )));
        }
        let column = find_filter_column(&table, &policy.column)?;
        warnings.extend(check_values(state, ws, &table, column, &policy.allowed_values).await?);
        policies.push(policy);
    }
    policies.sort();
    match state
        .store
        .replace_role_policies(ws, role_id, &policies)
        .await?
    {
        PoliciesOutcome::Done => Ok(SetPoliciesResponse { policies, warnings }),
        PoliciesOutcome::NotFound => Err(AppError::NotFound(
            "Không có vai dữ liệu này trong workspace".to_string(),
        )),
        PoliciesOutcome::AssignedWithoutRules => Err(AppError::BadRequest(
            "Vai đang gán cho thành viên phải còn ít nhất một luật; gỡ vai khỏi họ trước"
                .to_string(),
        )),
        PoliciesOutcome::SameRulesAs(name) => Err(AppError::BadRequest(format!(
            "Vai {name} đã có đúng bộ luật này; gán vai đó thay vì tạo vai trùng"
        ))),
    }
}

/// Cột chiều của phiên bản hiện hành của bảng table_id, kèm giá trị đã biết.
pub async fn filter_columns(
    state: &AppState,
    member: &Member,
    table_id: Uuid,
) -> Result<Vec<FilterColumnResponse>, AppError> {
    member.require_admin()?;
    let ws = &member.workspace;
    let table = load_policy_table(state, ws, table_id).await?;
    let dimensions: Vec<&ProfileColumn> = table
        .columns
        .iter()
        .filter(|c| c.role == ProfileRole::Dimension.as_str())
        .collect();
    let ids: Vec<Uuid> = dimensions.iter().map(|c| c.id).collect();
    // list_column_values trả theo tần suất giảm dần, nên giá trị nhiều dòng nhất đứng trước.
    let values = state.store.list_column_values(ws, &ids).await?;
    Ok(dimensions
        .into_iter()
        .map(|c| FilterColumnResponse {
            name: c.name.clone(),
            label: c.aliases.first().cloned(),
            values: values
                .iter()
                .filter(|v| v.column_id == c.id && !c.is_sensitive)
                .map(|v| v.value.clone())
                .collect(),
        })
        .collect())
}

/// Bỏ khoảng trắng hai đầu tên cột và giá trị, bỏ giá trị rỗng và lặp, sắp xếp.
fn build_clean_policy(policy: RolePolicy) -> RolePolicy {
    let values: BTreeSet<String> = policy
        .allowed_values
        .iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect();
    RolePolicy {
        table_id: policy.table_id,
        column: policy.column.trim().to_string(),
        allowed_values: values.into_iter().collect(),
    }
}

async fn load_policy_table(
    state: &AppState,
    ws: &WorkspaceId,
    table_id: Uuid,
) -> Result<PolicyTable, AppError> {
    let Some(dataset) = state.store.get_dataset(ws, table_id).await? else {
        return Err(AppError::BadRequest(
            "Không có bảng này trong workspace; tải lại trang rồi chọn lại".to_string(),
        ));
    };
    let Some(version_id) = dataset.current_version_id else {
        return Err(AppError::BadRequest(format!(
            "Bảng {} chưa có dữ liệu; chụp hay nạp bảng trước khi đặt luật",
            dataset.name
        )));
    };
    Ok(PolicyTable {
        columns: state.store.list_columns(ws, version_id).await?,
        name: dataset.name,
    })
}

fn find_filter_column<'a>(
    table: &'a PolicyTable,
    name: &str,
) -> Result<&'a ProfileColumn, AppError> {
    let Some(column) = table.columns.iter().find(|c| c.name == name) else {
        return Err(AppError::BadRequest(format!(
            "Bảng {} không có cột {name}",
            table.name
        )));
    };
    if column.role != ProfileRole::Dimension.as_str() {
        return Err(AppError::BadRequest(format!(
            "Cột {name} của bảng {} không phải cột chia nhóm (chiều); luật lọc dòng chỉ đặt trên cột chiều",
            table.name
        )));
    }
    Ok(column)
}

/// Lời báo về giá trị của một luật; rỗng khi mọi giá trị có trong hồ sơ cột.
async fn check_values(
    state: &AppState,
    ws: &WorkspaceId,
    table: &PolicyTable,
    column: &ProfileColumn,
    values: &[String],
) -> Result<Vec<String>, AppError> {
    if values.is_empty() {
        return Ok(vec![format!(
            "Luật trên cột {} không có giá trị nào: người mang vai không thấy dòng nào của bảng {}",
            column.name, table.name
        )]);
    }
    if column.is_sensitive {
        return Ok(vec![format!(
            "Cột {} đánh dấu nhạy cảm nên hồ sơ không lưu giá trị để kiểm; xem lại chính tả của giá trị",
            column.name
        )]);
    }
    let known: BTreeSet<String> = state
        .store
        .list_column_values(ws, &[column.id])
        .await?
        .into_iter()
        .map(|v| v.value)
        .collect();
    let missing: Vec<&str> = values
        .iter()
        .filter(|v| !known.contains(*v))
        .map(String::as_str)
        .collect();
    if missing.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![format!(
        "Giá trị {} không có trong hồ sơ cột {} của bảng {}; gõ sai thì người mang vai không thấy các dòng đó",
        missing.join(", "),
        column.name,
        table.name
    )])
}
