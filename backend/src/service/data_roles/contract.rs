// SPDX-License-Identifier: Apache-2.0
/* service/data_roles/contract.rs - request và response của API vai dữ liệu (api/data_roles.rs). Hợp đồng chung với
frontend qua openapi.json; giải thích luật ghép vai ở docs/API.md và docs/ARCHITECTURE.md mục 4b. */

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::engine::store::RolePolicy;

/// Create or rename a data role.
#[derive(Debug, Deserialize, ToSchema)]
pub struct DataRoleRequest {
    /// 1 to 60 characters, unique in the workspace (case-insensitive), for example `Sales South`.
    pub name: String,
    /// Optional, at most 500 characters.
    #[serde(default)]
    pub description: Option<String>,
}

/// A data role with its row rules and the members who hold it.
#[derive(Debug, Serialize, ToSchema)]
pub struct DataRoleResponse {
    pub id: Uuid,
    pub name: String,
    #[schema(required)]
    pub description: Option<String>,
    /// Rules of the role, by table and column. Rules on one table all apply together (AND).
    pub policies: Vec<RolePolicy>,
    /// Account ids of the members who hold the role.
    pub member_ids: Vec<Uuid>,
}

/// The full rule set of a role; replaces the previous one.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SetPoliciesRequest {
    /// One rule per table and column. Each `column` must be a dimension column of the table's current version.
    pub policies: Vec<RolePolicy>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SetPoliciesResponse {
    /// Rules as saved: values trimmed, without duplicates, sorted.
    pub policies: Vec<RolePolicy>,
    /// Vietnamese notes for the admin, for example a value not found in the column profile. The rules are saved.
    pub warnings: Vec<String>,
}

/// The full set of data roles of a member; empty removes every role.
#[derive(Debug, Deserialize, ToSchema)]
pub struct MemberDataRolesRequest {
    pub role_ids: Vec<Uuid>,
}

/// A column a row rule can use: a dimension column of the table's current version.
#[derive(Debug, Serialize, ToSchema)]
pub struct FilterColumnResponse {
    pub name: String,
    /// Vietnamese label: the first alias written by the semantic profile; null until then.
    #[schema(required)]
    pub label: Option<String>,
    /// Known values of the column from its profile, most rows first; empty for a sensitive column or one with too
    /// many values to list. A rule may still use other values.
    pub values: Vec<String>,
}
