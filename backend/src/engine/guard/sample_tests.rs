// SPDX-License-Identifier: Apache-2.0
/* engine/guard/sample_tests.rs - các luật kích thước của guard (sizes.rs, depth.rs, pieces.rs, width.rs) trên 32 câu
phân tích dạng mô hình hay viết (R01 tới R32) và 18 câu tìm cách lách luật (B01 tới B16, kể cả B01b, B05b), trên bảng
dataco có 52 cột như DataCo. Mỗi câu ghi kết quả mong đợi; câu bị từ chối có chủ ý ghi lý do ngay cạnh. Câu lấy từ các
đợt review các luật này, để sửa luật sau này thấy ngay câu nào đổi kết quả. */

use std::path::PathBuf;

use super::{build_guarded_sql, AllowedTable, GuardError};

const COLS: &[&str] = &[
    "Type",
    "Days for shipping (real)",
    "Days for shipment (scheduled)",
    "Benefit per order",
    "Sales per customer",
    "Delivery Status",
    "Late_delivery_risk",
    "Category Id",
    "Category Name",
    "Customer City",
    "Customer Country",
    "Customer Email",
    "Customer Fname",
    "Customer Id",
    "Customer Lname",
    "Customer Segment",
    "Customer State",
    "Customer Street",
    "Customer Zipcode",
    "Department Id",
    "Department Name",
    "Latitude",
    "Longitude",
    "Market",
    "Order City",
    "Order Country",
    "Order Customer Id",
    "order date (DateOrders)",
    "Order Id",
    "Order Item Cardprod Id",
    "Order Item Discount",
    "Order Item Discount Rate",
    "Order Item Id",
    "Order Item Product Price",
    "Order Item Profit Ratio",
    "Order Item Quantity",
    "Sales",
    "Order Item Total",
    "Order Profit Per Order",
    "Order Region",
    "Order State",
    "Order Status",
    "Order Zipcode",
    "Product Card Id",
    "Product Category Id",
    "Product Description",
    "Product Image",
    "Product Name",
    "Product Price",
    "Product Status",
    "shipping date (DateOrders)",
    "Shipping Mode",
];

fn tables() -> Vec<AllowedTable> {
    vec![AllowedTable {
        name: "dataco".to_string(),
        columns: COLS.iter().map(|c| c.to_string()).collect(),
        parquet_dir: PathBuf::from("/data/dataco"),
        row_filters: vec![],
    }]
}

/// Kết quả guard thu gọn để so: chỉ loại, không so lý do.
#[derive(Debug, PartialEq)]
enum Verdict {
    Accept,
    Oversized,
    Rejected,
}

fn verdict(sql: &str) -> Verdict {
    match build_guarded_sql(sql, &tables(), 1000) {
        Ok(_) => Verdict::Accept,
        Err(GuardError::Oversized(_)) => Verdict::Oversized,
        Err(GuardError::Rejected(_) | GuardError::UnknownName(_)) => Verdict::Rejected,
        Err(GuardError::Internal(e)) => panic!("internal error: {e:?}"),
    }
}

const D: &str = r#"CAST(strptime("order date (DateOrders)", '%m/%d/%Y %H:%M') AS DATE)"#;

fn realistic() -> Vec<(&'static str, String)> {
    let d = D;
    let countries: Vec<String> = (0..60).map(|i| format!("'Country{i}'")).collect();
    let in_list = countries.join(", ");
    let many_case: String = (1..=40)
        .map(|i| format!("WHEN \"Order Country\" = 'C{i}' THEN 'Group{}'", i % 7))
        .collect::<Vec<_>>()
        .join(" ");
    vec![
        ("R01 doanh thu theo Market", r#"SELECT "Market", SUM("Order Item Total") AS revenue FROM dataco GROUP BY "Market" ORDER BY revenue DESC"#.into()),
        ("R02 CTE + tỉ trọng bằng câu con vô hướng", format!(r#"WITH d AS (SELECT {d} AS order_day, "Market", "Order Item Total" FROM dataco), m AS (SELECT "Market", SUM("Order Item Total") AS revenue FROM d WHERE order_day >= DATE '2017-01-01' GROUP BY "Market") SELECT "Market", revenue, revenue / (SELECT SUM(revenue) FROM m) AS share FROM m ORDER BY revenue DESC"#)),
        ("R03 tăng trưởng tháng (3 CTE + lag)", format!(r#"WITH parsed AS (SELECT {d} AS order_day, "Order Item Total" AS total FROM dataco), monthly AS (SELECT date_trunc('month', order_day) AS month, SUM(total) AS revenue FROM parsed GROUP BY 1), growth AS (SELECT month, revenue, lag(revenue) OVER (ORDER BY month) AS prev FROM monthly) SELECT strftime(month, '%Y-%m') AS month, revenue, round(100.0 * (revenue - prev) / prev, 2) AS mom_pct FROM growth ORDER BY month"#)),
        ("R04 top 3 sản phẩm mỗi vùng theo tăng trưởng năm (4 CTE)", format!(r#"WITH parsed AS (SELECT {d} AS order_day, "Order Region", "Product Name", "Order Item Total" FROM dataco), yearly AS (SELECT "Order Region", "Product Name", year(order_day) AS yr, SUM("Order Item Total") AS revenue FROM parsed GROUP BY ALL), yoy AS (SELECT *, revenue - lag(revenue) OVER (PARTITION BY "Order Region", "Product Name" ORDER BY yr) AS delta FROM yearly), ranked AS (SELECT *, row_number() OVER (PARTITION BY "Order Region" ORDER BY delta DESC) AS rn FROM yoy WHERE yr = 2017) SELECT "Order Region", "Product Name", delta FROM ranked WHERE rn <= 3 ORDER BY "Order Region", rn"#)),
        ("R05 top 3 mỗi vùng bằng QUALIFY", r#"SELECT "Order Region", "Product Name", SUM("Order Item Total") AS revenue FROM dataco GROUP BY ALL QUALIFY row_number() OVER (PARTITION BY "Order Region" ORDER BY revenue DESC) <= 3"#.into()),
        ("R06 câu con trong FROM 2 tầng + WHERE IN câu con", r#"SELECT t."Market", t.avg_order FROM (SELECT "Market", AVG(order_total) AS avg_order FROM (SELECT "Market", "Order Id", SUM("Order Item Total") AS order_total FROM dataco GROUP BY 1, 2) o GROUP BY 1) t WHERE t."Market" IN (SELECT "Market" FROM dataco WHERE "Order Status" = 'COMPLETE' GROUP BY 1 HAVING COUNT(*) > 100) ORDER BY 2 DESC"#.into()),
        ("R07 CASE 40 nhánh", format!(r#"SELECT CASE {many_case} ELSE 'Other' END AS grp, SUM("Order Item Total") AS revenue FROM dataco GROUP BY 1"#)),
        ("R08 IN 60 giá trị", format!(r#"SELECT "Order Country", COUNT(*) FROM dataco WHERE "Order Country" IN ({in_list}) GROUP BY 1"#)),
        ("R09 UNION ALL dòng tổng", r#"SELECT "Market", SUM("Order Item Total") AS revenue FROM dataco GROUP BY 1 UNION ALL SELECT 'Tổng', SUM("Order Item Total") FROM dataco ORDER BY revenue DESC"#.into()),
        ("R10 UNION 4 nhánh trong ngoặc, mỗi nhánh LIMIT", r#"(SELECT 'top' AS kind, "Product Name", SUM("Order Item Total") AS r FROM dataco GROUP BY 2 ORDER BY r DESC LIMIT 5) UNION ALL (SELECT 'bottom', "Product Name", SUM("Order Item Total") AS r FROM dataco GROUP BY 2 ORDER BY r ASC LIMIT 5) UNION ALL (SELECT 'top_profit', "Product Name", SUM("Order Profit Per Order") AS r FROM dataco GROUP BY 2 ORDER BY r DESC LIMIT 5) UNION ALL (SELECT 'loss', "Product Name", SUM("Order Profit Per Order") AS r FROM dataco GROUP BY 2 ORDER BY r ASC LIMIT 5)"#.into()),
        ("R11 strftime năm-tháng", format!(r#"SELECT strftime({d}, '%Y-%m') AS ym, SUM("Order Item Total") FROM dataco GROUP BY 1 ORDER BY 1"#)),
        ("R12 strftime kiểu SQLite (định dạng trước)", format!(r#"SELECT strftime('%Y-%m', {d}) AS ym, SUM("Order Item Total") FROM dataco GROUP BY 1 ORDER BY 1"#)),
        ("R13 nhãn quý ghép ||", format!(r#"SELECT year({d}) || '-Q' || quarter({d}) AS q, SUM("Order Item Total") FROM dataco GROUP BY 1 ORDER BY 1"#)),
        ("R14 làm sạch chuỗi", r#"SELECT lower(trim(regexp_replace("Customer City", '\s+', ' ', 'g'))) AS city, COUNT(*) FROM dataco GROUP BY 1 ORDER BY 2 DESC"#.into()),
        ("R15 họ tên + địa chỉ concat_ws", r#"SELECT "Customer Fname" || ' ' || "Customer Lname" AS customer, concat_ws(', ', "Customer Street", "Customer City", "Customer State", "Customer Country") AS address, SUM("Order Item Total") AS spent FROM dataco GROUP BY 1, 2 ORDER BY spent DESC LIMIT 10"#.into()),
        ("R16 lpad mã tháng", format!(r#"SELECT year({d}) || '-' || lpad(CAST(month({d}) AS VARCHAR), 2, '0') AS ym, COUNT(*) FROM dataco GROUP BY 1"#)),
        ("R17 pivot tay 12 tháng x 3 chỉ số (36 cột)", {
            let cols: Vec<String> = (1..=12).flat_map(|m| vec![
                format!(r#"SUM(CASE WHEN month({d}) = {m} THEN "Order Item Total" END) AS rev_{m}"#),
                format!(r#"COUNT(DISTINCT CASE WHEN month({d}) = {m} THEN "Order Id" END) AS orders_{m}"#),
                format!(r#"AVG(CASE WHEN month({d}) = {m} THEN "Order Item Discount Rate" END) AS disc_{m}"#),
            ]).collect();
            format!(r#"SELECT "Market", {} FROM dataco GROUP BY 1"#, cols.join(", "))
        }),
        ("R18 FILTER và tỉ lệ giao trễ", r#"SELECT "Shipping Mode", COUNT(*) FILTER (WHERE "Late_delivery_risk" = 1) * 1.0 / COUNT(*) AS late_rate FROM dataco GROUP BY 1"#.into()),
        ("R19 cột so với trung bình bằng câu con tương quan", r#"SELECT "Category Name", AVG("Order Item Total") AS avg_total FROM dataco d1 GROUP BY 1 HAVING AVG("Order Item Total") > (SELECT AVG("Order Item Total") FROM dataco) ORDER BY 2 DESC"#.into()),
        ("R20 EXISTS tương quan", r#"SELECT DISTINCT "Customer Id" FROM dataco a WHERE EXISTS (SELECT 1 FROM dataco b WHERE b."Customer Id" = a."Customer Id" AND b."Order Status" = 'CANCELED')"#.into()),
        ("R21 IN nhiều cột (tuple), 10 cặp", {
            let pairs: Vec<String> = (0..10).map(|i| format!("('M{i}', 'R{i}')")).collect();
            format!(r#"SELECT "Market", "Order Region", SUM("Order Item Total") FROM dataco WHERE ("Market", "Order Region") IN ({}) GROUP BY 1, 2"#, pairs.join(", "))
        }),
        ("R22 = ANY(ARRAY[...]) 25 giá trị kiểu Postgres", {
            let v: Vec<String> = (0..25).map(|i| format!("'C{i}'")).collect();
            format!(r#"SELECT COUNT(*) FROM dataco WHERE "Order Country" = ANY(ARRAY[{}])"#, v.join(", "))
        }),
        ("R23 list literal IN 25 giá trị kiểu DuckDB", {
            let v: Vec<String> = (0..25).map(|i| format!("'C{i}'")).collect();
            format!(r#"SELECT COUNT(*) FROM dataco WHERE list_contains([{}], "Order Country")"#, v.join(", "))
        }),
        ("R24 quantile nhiều mức", r#"SELECT "Market", quantile_cont("Order Item Total", [0.1, 0.25, 0.5, 0.75, 0.9]) AS q FROM dataco GROUP BY 1"#.into()),
        ("R25 4 CTE: lọc, khách, phân hạng RFM, tổng hợp", format!(r#"WITH base AS (SELECT "Customer Id", {d} AS order_day, "Order Id", "Order Item Total" FROM dataco WHERE "Order Status" NOT IN ('CANCELED', 'SUSPECTED_FRAUD')), cust AS (SELECT "Customer Id", MAX(order_day) AS last_order, COUNT(DISTINCT "Order Id") AS freq, SUM("Order Item Total") AS monetary FROM base GROUP BY 1), scored AS (SELECT *, ntile(5) OVER (ORDER BY last_order) AS r, ntile(5) OVER (ORDER BY freq) AS f, ntile(5) OVER (ORDER BY monetary) AS m FROM cust), seg AS (SELECT *, CASE WHEN r >= 4 AND f >= 4 THEN 'Champions' WHEN r <= 2 AND f >= 3 THEN 'At risk' ELSE 'Others' END AS segment FROM scored) SELECT segment, COUNT(*) AS customers, AVG(monetary) AS avg_value FROM seg GROUP BY 1 ORDER BY 2 DESC"#)),
        ("R26 WHERE theo khoảng ngày + BETWEEN", format!(r#"SELECT "Order Region", SUM("Order Item Total") FROM dataco WHERE {d} BETWEEN DATE '2017-01-01' AND DATE '2017-12-31' GROUP BY 1"#)),
        ("R27 tỉ trọng bằng window (không câu con)", r#"SELECT "Market", SUM("Order Item Total") AS revenue, SUM("Order Item Total") / SUM(SUM("Order Item Total")) OVER () AS share FROM dataco GROUP BY 1"#.into()),
        ("R28 JOIN bảng tạm với chính nó (năm nay vs năm trước)", format!(r#"WITH y AS (SELECT "Market", year({d}) AS yr, SUM("Order Item Total") AS revenue FROM dataco GROUP BY 1, 2) SELECT a."Market", a.revenue, b.revenue AS prev, a.revenue / b.revenue - 1 AS growth FROM y a JOIN y b ON a."Market" = b."Market" AND a.yr = b.yr + 1 WHERE a.yr = 2017"#)),
        ("R29 mô tả dài trong CASE nhãn (nhãn 1200 byte)", format!(r#"SELECT CASE WHEN "Order Status" = 'COMPLETE' THEN '{}' ELSE 'x' END FROM dataco LIMIT 1"#, "a".repeat(1200))),
        ("R30 nhãn tiếng Việt nhiều ghép (5 cột nhãn, mỗi cột 5 mảnh)", format!(r#"SELECT 'Tháng ' || month({d}) || '/' || year({d}) || ' - ' AS l1, 'Quý ' || quarter({d}) || '/' || year({d}) || ':' AS l2, "Customer Fname" || ' ' || "Customer Lname" || ' (' || "Customer Segment" AS l3, "Order City" || ', ' || "Order State" || ', ' || "Order Country" AS l4, COUNT(*) FROM dataco GROUP BY ALL"#)),
        ("R31 CTE + câu con trong CTE + câu con vô hướng ở SELECT ngoài", String::from(r#"WITH orders AS (SELECT "Order Id", "Market", SUM("Order Item Total") AS total FROM dataco WHERE "Order Status" IN (SELECT "Order Status" FROM dataco WHERE "Order Status" <> 'CANCELED' GROUP BY 1) GROUP BY 1, 2), stats AS (SELECT "Market", AVG(total) AS avg_total FROM orders GROUP BY 1) SELECT "Market", avg_total, avg_total / (SELECT AVG(avg_total) FROM stats) AS idx FROM stats ORDER BY idx DESC"#)),
        ("R32 bảng tạm 4 tầng không ai dùng", r#"WITH a AS (SELECT v FROM (SELECT v FROM (SELECT "Market" AS v FROM (SELECT "Market" FROM dataco) t1) t2) t3) SELECT 1 AS k"#.into()),
    ]
}

fn bypasses() -> Vec<(&'static str, String)> {
    let lit = "x".repeat(1000);
    let cols200: Vec<String> = (1..=200).map(|i| format!("v AS c{i}")).collect();
    let cols200 = cols200.join(", ");
    let joins: Vec<String> = (1..=100).map(|i| format!("w a{i}")).collect();
    let joins = joins.join(", ");
    vec![
        // Depth: bảng tạm trùng tên trong WITH lồng ghi đè số tầng.
        ("B01 sâu 7 tầng qua bóng tên bảng tạm", r#"WITH c1 AS (SELECT 'ab' AS v), c2 AS (SELECT v || v || v AS v FROM c1), c3 AS (SELECT v || v || v AS v FROM c2), r3 AS (WITH c3 AS (SELECT 'z' AS v) SELECT v FROM c3), c4 AS (SELECT v || v || v AS v FROM c3), r4 AS (WITH c4 AS (SELECT 'z' AS v) SELECT v FROM c4), c5 AS (SELECT v || v || v AS v FROM c4), r5 AS (WITH c5 AS (SELECT 'z' AS v) SELECT v FROM c5), c6 AS (SELECT v || v || v AS v FROM c5), r6 AS (WITH c6 AS (SELECT 'z' AS v) SELECT v FROM c6), c7 AS (SELECT v || v || v AS v FROM c6) SELECT length(v) FROM c7"#.into()),
        ("B01b cùng câu không có bóng tên (đối chứng)", r#"WITH c1 AS (SELECT 'ab' AS v), c2 AS (SELECT v || v || v AS v FROM c1), c3 AS (SELECT v || v || v AS v FROM c2), c4 AS (SELECT v || v || v AS v FROM c3), c5 AS (SELECT v || v || v AS v FROM c4), c6 AS (SELECT v || v || v AS v FROM c5), c7 AS (SELECT v || v || v AS v FROM c6) SELECT length(v) FROM c7"#.into()),
        // Pieces: bí danh cột dùng lại trong cùng SELECT (lateral alias) cho 3^6*2 = 1458 lần với 20 mảnh, 1 tầng.
        ("B02 bí danh cột dùng lại trong cùng SELECT", r#"SELECT 'ab' AS a0, a0 || a0 || a0 AS a1, a1 || a1 || a1 AS a2, a2 || a2 || a2 AS a3, a3 || a3 || a3 AS a4, a4 || a4 || a4 AS a5, a5 || a5 || a5 AS a6, a6 || a6 AS a7, length(a7) AS n"#.into()),
        // Width: * qua JOIN bảng tạm với chính nó.
        ("B03 * qua 100 lần JOIN bảng tạm 200 cột (20 000 cột)", format!("WITH w AS (SELECT {cols200} FROM (SELECT '{lit}' AS v)) SELECT * FROM {joins}")),
        ("B04 SELECT không FROM, chuỗi 1000 byte", format!("SELECT '{lit}' AS v")),
        ("B05 lpad ký tự 4 byte", r#"SELECT octet_length(lpad('😀', 1000, '😀'))"#.into()),
        ("B05b lpad ký tự 4 byte (length thay octet_length)", r#"SELECT strlen_ok FROM (SELECT length(lpad('😀', 1000, '😀')) AS strlen_ok)"#.into()),
        ("B06 struct qua tuple trong IN không đếm? (IN list)", format!("SELECT v IN ({}) FROM (SELECT 'a' AS v)", (0..50).map(|i| format!("'{i}'")).collect::<Vec<_>>().join(","))),
        ("B07 string_agg trên UNION ALL hằng", format!("WITH u AS (SELECT '{lit}' AS v UNION ALL SELECT '{lit}' UNION ALL SELECT '{lit}') SELECT length(string_agg(v, '')) FROM u")),
        ("B08 ROW(...)", "SELECT ROW('a', 'b')".into()),
        ("B09 concat dạng phương thức 'a'.concat(...)", "SELECT ('ab').concat('cd', 'ef') AS x".into()),
        ("B10 Expr::Nested (a || b) || (c || d)", "SELECT ('a' || 'b') || ('c' || 'd') AS x".into()),
        ("B11 ORDER BY câu con 5 tầng", "SELECT 1 AS k ORDER BY (SELECT (SELECT (SELECT (SELECT (SELECT 1)))))".into()),
        ("B12 JOIN ON câu con 5 tầng", r#"SELECT a."Market" FROM dataco a JOIN dataco b ON a."Order Id" = (SELECT (SELECT (SELECT (SELECT max("Order Id") FROM dataco)))) LIMIT 1"#.into()),
        ("B13 LATERAL", r#"SELECT a."Market", x.n FROM dataco a, LATERAL (SELECT a."Order Id" AS n) x LIMIT 1"#.into()),
        ("B14 alias cột của CTE (WITH t(v))", String::from("WITH t(v) AS (SELECT 'ab') SELECT v || v AS v2 FROM t")),
        ("B15 strftime %c x50", format!("SELECT strftime(TIMESTAMP '2020-01-01 10:00:00', '{}')", "%c".repeat(50))),
        ("B16 E-string và \\u escape độ dài sau giải mã", format!("SELECT E'{}'", "\\x41".repeat(260))),
    ]
}

/// Câu phân tích thường gặp qua guard; sáu câu bị từ chối có chủ ý, lý do ghi cạnh. Câu Oversized vào vòng sửa, mô
/// hình viết lại; câu Rejected dừng.
#[test]
fn realistic_queries() {
    use Verdict::*;
    let expected = [
        ("R01", Accept),
        ("R02", Accept),
        ("R03", Accept),
        // 4 bảng tạm nối nhau rồi câu chính: 5 tầng. Trần 4 giữ vì 6 tầng cho trường hợp xấu nhất đo được 486 MB (chép
        // 200 cột) và 2,4 GB (UNION ALL); xem docs/GUARD.md, mục Số tầng, mảnh ghép, độ rộng.
        ("R04", Oversized),
        ("R05", Accept),
        ("R06", Accept),
        ("R07", Accept),
        ("R08", Accept),
        ("R09", Accept),
        ("R10", Accept),
        ("R11", Accept),
        // strftime(định dạng, ngày) kiểu SQLite: luật strftime chỉ nhận thứ tự của DuckDB, lý do nói rõ thứ tự.
        ("R12", Oversized),
        ("R13", Accept),
        ("R14", Accept),
        ("R15", Accept),
        ("R16", Accept),
        ("R17", Accept),
        ("R18", Accept),
        ("R19", Accept),
        ("R20", Accept),
        ("R21", Accept),
        ("R22", Accept),
        // list_contains không có trong danh sách hàm được phép (có từ trước các luật kích thước).
        ("R23", Rejected),
        ("R24", Accept),
        // Cùng dạng R04: 5 tầng.
        ("R25", Oversized),
        ("R26", Accept),
        ("R27", Accept),
        ("R28", Accept),
        // Nhãn 1200 byte vượt trần chuỗi hằng 1000 byte.
        ("R29", Oversized),
        ("R30", Accept),
        // Bảng tạm có câu con, bảng tạm thứ hai đọc nó, câu chính có câu con vô hướng đọc bảng tạm thứ hai: 5 tầng.
        ("R31", Oversized),
        // Bảng tạm 4 tầng mà câu chính không đọc vẫn tính vào số tầng (5): đếm bảo thủ, xem chú thích ở depth.rs.
        ("R32", Oversized),
    ];
    check(realistic(), &expected);
}

/// Cách lách các luật: đều bị chặn, trừ các câu vô hại (chuỗi 1000 byte, IN dài, string_agg tính lúc chạy, LATERAL).
#[test]
fn bypass_attempts() {
    use Verdict::*;
    let expected = [
        // Bảng tạm cùng tên trong WITH lồng không che số tầng của bảng tạm ngoài: 8 tầng thật.
        ("B01 ", Oversized),
        ("B01b", Oversized),
        // Bí danh dùng lại trong cùng danh sách cột cộng mảnh của bí danh.
        ("B02", Oversized),
        // 100 nguồn trong một FROM.
        ("B03", Oversized),
        ("B04", Accept),
        // octet_length không có trong danh sách hàm.
        ("B05 ", Rejected),
        // lpad tối đa 250 ký tự.
        ("B05b", Oversized),
        ("B06", Accept),
        // Tính lúc chạy, trong trần bộ nhớ của engine/query.rs.
        ("B07", Accept),
        ("B08", Rejected),
        // Gọi hàm dạng phương thức.
        ("B09", Oversized),
        ("B10", Accept),
        ("B11", Oversized),
        ("B12", Oversized),
        ("B13", Accept),
        ("B14", Accept),
        ("B15", Accept),
        // sqlparser không đọc E'...' ở cách đọc DuckDB: thành tên cột E không có.
        ("B16", Rejected),
    ];
    check(bypasses(), &expected);
}

fn check(queries: Vec<(&'static str, String)>, expected: &[(&str, Verdict)]) {
    assert_eq!(queries.len(), expected.len());
    for ((label, sql), (prefix, want)) in queries.iter().zip(expected) {
        assert!(label.starts_with(prefix), "{label} vs {prefix}");
        assert_eq!(&verdict(sql), want, "{label}");
    }
}
