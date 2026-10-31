// SPDX-License-Identifier: Apache-2.0
/* service/domains/contract.rs - kiểu request và response của API khái niệm ngành (api/domains.rs): các gói của
workspace kèm mọi khái niệm, khái niệm admin gửi lên. Cách tính và luật kiểm dùng thẳng kiểu của engine/domains.rs,
cùng dạng với file gói (docs/DOMAINS.md). Mô tả trong đặc tả OpenAPI viết tiếng Anh như các kiểu khác của API. */

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::engine::domains::{Calculation, SanityCheck};
use crate::engine::store::DomainConceptRecord;

/// A business concept of an industry pack in this workspace.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Concept {
    pub id: Uuid,
    /// Id of the concept in the pack file (`data/domains/<pack>.json`); null for a concept an admin added.
    #[schema(required)]
    pub key: Option<String>,
    pub name: String,
    pub description: String,
    /// Other words for the concept, Vietnamese and English; the prompt lists them next to the name.
    pub synonyms: Vec<String>,
    pub calculations: Vec<Calculation>,
    pub dimensions: Vec<String>,
    pub sanity_checks: Vec<SanityCheck>,
    /// False for a concept an admin turned off; it stays here but is left out of the prompt.
    pub enabled: bool,
    /// Last edit on the concepts screen; null when unchanged since it was copied from the pack file.
    #[schema(required)]
    pub updated_at: Option<DateTime<Utc>>,
}

impl From<DomainConceptRecord> for Concept {
    fn from(r: DomainConceptRecord) -> Concept {
        Concept {
            id: r.id,
            key: r.key,
            name: r.name,
            description: r.description,
            synonyms: r.synonyms,
            calculations: r.calculations.0,
            dimensions: r.dimensions,
            sanity_checks: r.sanity_checks.0,
            enabled: r.enabled,
            updated_at: r.updated_at,
        }
    }
}

/// An industry pack of the workspace with all its concepts, enabled or not.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ConceptPack {
    /// Pack id, the file name without `.json`; a dataset uses the pack whose key is in its `domain`.
    pub key: String,
    pub name: String,
    pub description: String,
    /// In prompt order: the order of the pack file, concepts added by admins last.
    pub concepts: Vec<Concept>,
}

/// Industry packs of the workspace, sorted by key.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ConceptsResponse {
    pub packs: Vec<ConceptPack>,
}

/// A concept to add, or the new content of one. Errors name the wrong field in `field`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ConceptRequest {
    /// Unique in the pack, compared ignoring case and where the tone mark sits (not ignoring tone marks).
    pub name: String,
    pub description: String,
    /// At least one; a word may belong to only one concept of the pack, compared without case and accents.
    pub synonyms: Vec<String>,
    /// At most three.
    #[serde(default)]
    pub calculations: Vec<Calculation>,
    #[serde(default)]
    pub dimensions: Vec<String>,
    #[serde(default)]
    pub sanity_checks: Vec<SanityCheck>,
    /// Defaults to true.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}
