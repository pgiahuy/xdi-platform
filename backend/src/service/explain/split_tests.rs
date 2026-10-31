// SPDX-License-Identifier: Apache-2.0
/* service/explain/split_tests.rs - test tách chênh lệch (explain/split.rs, #201): có lượng thì dùng đúng số của
volume_price kèm phần trăm mỗi phần; không có lượng thì không tách (không tách theo số dòng); hai phần ngược chiều thì
không có phần trăm. */

use crate::service::ask::contract::Explanation;
use crate::service::ask::examples::build_why_example;
use crate::service::explain::split::build_split;

fn example() -> Explanation {
    build_why_example().explanation.expect("why example")
}

#[test]
fn volume_price_is_used_with_its_shares() {
    let e = example();
    let split = build_split(&e).expect("split");
    let vp = e.volume_price.as_ref().expect("volume_price");
    assert_eq!(split.volume_label, vp.volume_label);
    assert_eq!(split.numbers.volume_effect, vp.volume_effect);
    assert_eq!(split.numbers.price_effect, vp.price_effect);
    // Số đơn giảm góp 1.117.725 trên 1.283.400 của mức giảm.
    assert_eq!(split.volume_share.as_deref(), Some("87.1"));
    assert_eq!(split.price_share.as_deref(), Some("12.9"));
}

#[test]
fn no_volume_means_no_split() {
    let mut e = example();
    e.volume_price = None;
    assert!(build_split(&e).is_none());
}

#[test]
fn parts_going_opposite_ways_have_no_share() {
    let mut e = example();
    let vp = e.volume_price.as_mut().expect("volume_price");
    // Số đơn giảm nhiều hơn cả mức giảm, giá trị mỗi đơn kéo lại một phần.
    vp.volume_effect = "-1500000.00".to_string();
    vp.price_effect = "216600.00".to_string();
    let split = build_split(&e).expect("split");
    assert_eq!((split.volume_share, split.price_share), (None, None));
}
