// SPDX-License-Identifier: Apache-2.0
/* engine/guard/join_pairs.rs - kiểm từng cặp nguồn đã nối của luật nối bảng (guard/joins.rs): hai bảng giữ nguyên dòng
phải nối đúng cột của một khóa (check_strict), nối có nguồn đã gộp nhóm hay nửa nối chỉ cần cột cùng miền giá trị
(check_loose), và mọi bảng của một FROM phải nối với nhau (check_connected). Phép bằng gom ở guard/join_conditions.rs. */

use std::collections::BTreeMap;

use super::join_conditions::{Conditions, Equality};
use super::join_keys::{Keys, Origin};
use super::join_sources::Source;

/// Hai bảng giữ nguyên dòng nối với nhau: các phép bằng phải đúng bằng cột của một khóa ngoại giữa hai bảng.
pub(super) fn check_strict(
    tx: &str,
    ty: &str,
    x: (usize, usize),
    group: &[&Equality],
    keys: &Keys,
) -> Result<(), String> {
    let mut pairs = Vec::new();
    for equality in group {
        // Đặt cột của nguồn x trước, vì câu viết a.c = b.d hay b.d = a.c đều được.
        let (on_x, on_y) = if (equality.a.level, equality.a.index) == x {
            (&equality.a.origin, &equality.b.origin)
        } else {
            (&equality.b.origin, &equality.a.origin)
        };
        match (on_x, on_y) {
            (Some(ox), Some(oy)) => pairs.push((ox.column.clone(), oy.column.clone())),
            // Cột tính ra trong câu con chỉ lọc một bảng: không phải cột khóa.
            _ => pairs.push((String::new(), String::new())),
        }
    }
    if keys.key_matches(tx, ty, &pairs) {
        return Ok(());
    }
    let written: Vec<&str> = group.iter().map(|e| e.text.as_str()).collect();
    Err(format!(
        "Nối bảng {tx} với {ty} sai điều kiện ({}). {}",
        written.join(" AND "),
        keys.describe_keys(tx, ty)
    ))
}

pub(super) fn build_origin_pairs(group: &[&Equality]) -> Vec<(Origin, Origin, String)> {
    group
        .iter()
        .filter_map(|e| match (&e.a.origin, &e.b.origin) {
            (Some(a), Some(b)) => Some((a.clone(), b.clone(), e.text.clone())),
            _ => None,
        })
        .collect()
}

/// Nối có bảng tạm, câu con đã gộp nhóm, hay nửa nối (IN, EXISTS, câu con tương quan): mỗi phép bằng có cột gốc biết
/// được phải so hai cột cùng miền giá trị (Keys::same_domain), và các cặp thuộc khóa ngoại phải đủ cột của khóa đó.
/// Cột tính ra (tháng, năm) không kiểm: nó không phải cột của bảng nào.
pub(super) fn check_loose(pairs: &[(Origin, Origin, String)], keys: &Keys) -> Result<(), String> {
    let mut by_tables: BTreeMap<(&str, &str), Vec<(String, String)>> = BTreeMap::new();
    for (a, b, text) in pairs {
        if !keys.same_domain(a, b) {
            return Err(format!(
                "Phép bằng {text} so cột {a} với {b}, hai cột này không nối với nhau qua khóa ngoại nào. {}",
                keys.describe_keys(&a.table, &b.table)
            ));
        }
        if a != b && keys.is_key_pair(a, b) {
            let pair = (a.column.clone(), b.column.clone());
            by_tables
                .entry((&a.table, &b.table))
                .or_default()
                .push(pair);
        }
    }
    for ((ta, tb), pairs) in &by_tables {
        if !keys.key_matches(ta, tb, pairs) {
            return Err(format!(
                "Câu chỉ so một phần khóa ngoại giữa {ta} và {tb}. {}",
                keys.describe_keys(ta, tb)
            ));
        }
    }
    Ok(())
}

/// Mọi bảng giữ nguyên dòng trong FROM phải nối với nhau qua phép bằng, trực tiếp hay qua nguồn khác. Thiếu thì là
/// tích chéo: mỗi dòng bảng này ghép với mọi dòng bảng kia.
pub(super) fn check_connected(
    conditions: &Conditions,
    sources: &[Source],
    own: usize,
    keys: &Keys,
) -> Result<(), String> {
    let mut group: Vec<usize> = (0..sources.len()).collect();
    let mut links: Vec<(usize, usize)> = conditions.edges.clone();
    for e in &conditions.equalities {
        if e.a.level == own && e.b.level == own {
            links.push((e.a.index, e.b.index));
        }
    }
    // Gộp nhóm đơn giản: lặp tới khi không đổi; số nguồn tối đa 6 (guard/width.rs) nên không cần cấu trúc nhanh hơn.
    let mut changed = true;
    while changed {
        changed = false;
        for &(x, y) in &links {
            let low = group[x].min(group[y]);
            if group[x] != low || group[y] != low {
                group[x] = low;
                group[y] = low;
                changed = true;
            }
        }
    }
    let rows: Vec<usize> = (0..sources.len())
        .filter(|&i| sources[i].rows_table().is_some())
        .collect();
    let Some(&first) = rows.first() else {
        return Ok(());
    };
    let Some(&apart) = rows.iter().find(|&&i| group[i] != group[first]) else {
        return Ok(());
    };
    let (tx, ty) = (
        sources[first].rows_table().unwrap_or_default(),
        sources[apart].rows_table().unwrap_or_default(),
    );
    Err(format!(
        "Bảng {tx} và {ty} được nối mà không có phép bằng nào giữa cột của hai bảng (CROSS JOIN, ON true hay thiếu \
         điều kiện nối trong WHERE). {}",
        keys.describe_keys(tx, ty)
    ))
}
