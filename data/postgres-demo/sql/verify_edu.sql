-- sql/verify_edu.sql - kiểm dữ liệu của edu_demo sau khi nạp, so với bảng staging (bản chép nguyên CSV OULAD).
-- Cần nạp với KEEP_STAGING=1: KEEP_STAGING=1 sh load.sh edu, rồi
--   docker compose exec -T demo-db psql -U demo -d edu_demo -f - < sql/verify_edu.sql
-- Mỗi câu in cột ok; mọi dòng phải là t. Câu 1, 3, 4, 5 so dữ liệu thật; câu 2 đếm khóa ngoại; câu 6 kiểm bước cài
-- nguyên nhân: đổi đúng 381 sinh viên đã định, đổi đúng như mô tả. Phần DataCo ở sql/verify_sales.sql.

\pset pager off

\echo '1. Số dòng so với CSV'
SELECT check_name, db, csv, db = csv AS ok FROM (
SELECT 'courses = dòng CSV' AS check_name, (SELECT count(*) FROM edu.courses) AS db, (SELECT count(*) FROM staging.oulad_courses) AS csv
UNION ALL SELECT 'students = dòng CSV', (SELECT count(*) FROM edu.students), (SELECT count(*) FROM staging.oulad_student_info)
UNION ALL SELECT 'registrations = dòng CSV', (SELECT count(*) FROM edu.registrations), (SELECT count(*) FROM staging.oulad_student_registration)
UNION ALL SELECT 'assessments = dòng CSV', (SELECT count(*) FROM edu.assessments), (SELECT count(*) FROM staging.oulad_assessments)
UNION ALL SELECT 'student_assessments = dòng CSV', (SELECT count(*) FROM edu.student_assessments), (SELECT count(*) FROM staging.oulad_student_assessment)
UNION ALL SELECT 'vle_materials = dòng CSV', (SELECT count(*) FROM edu.vle_materials), (SELECT count(*) FROM staging.oulad_vle)
UNION ALL SELECT 'tổng sum_click = CSV', (SELECT sum(sum_click) FROM edu.student_daily_clicks), (SELECT sum(sum_click::bigint) FROM staging.oulad_student_vle)
UNION ALL SELECT 'tổng num_sites = (sinh viên, tài liệu, ngày) khác nhau', (SELECT sum(num_sites) FROM edu.student_daily_clicks),
    (SELECT count(*) FROM (SELECT DISTINCT code_module, code_presentation, id_student, id_site, date FROM staging.oulad_student_vle) s)
) counts;

\echo '2. Khóa ngoại: đủ 7 ràng buộc'
\echo '   Không kiểm convalidated: khóa ngoại khai trong CREATE TABLE luôn là true vì được kiểm ở mỗi lần INSERT'
SELECT count(*) AS foreign_keys, count(*) = 7 AS ok
FROM pg_constraint
WHERE contype = 'f' AND connamespace = 'edu'::regnamespace;

\echo '3. Nối bảng sự kiện nhiều-một với mọi bảng chiều: số dòng không đổi'
SELECT fact, joined_rows, fact_rows, joined_rows = fact_rows AS ok FROM (
SELECT 'edu.student_assessments' AS fact, count(*) AS joined_rows, (SELECT count(*) FROM edu.student_assessments) AS fact_rows
FROM edu.student_assessments sa
JOIN edu.assessments a ON a.id_assessment = sa.id_assessment
JOIN edu.students s ON (s.code_module, s.code_presentation, s.id_student) = (sa.code_module, sa.code_presentation, sa.id_student)
JOIN edu.courses co ON (co.code_module, co.code_presentation) = (s.code_module, s.code_presentation)
JOIN edu.registrations r ON (r.code_module, r.code_presentation, r.id_student) = (s.code_module, s.code_presentation, s.id_student)
UNION ALL
SELECT 'edu.student_daily_clicks', count(*), (SELECT count(*) FROM edu.student_daily_clicks)
FROM edu.student_daily_clicks dc
JOIN edu.students s ON (s.code_module, s.code_presentation, s.id_student) = (dc.code_module, dc.code_presentation, dc.id_student)
JOIN edu.courses co ON (co.code_module, co.code_presentation) = (s.code_module, s.code_presentation)
) joined;

\echo '4. Môn và kỳ chép vào student_assessments khớp bài đánh giá'
SELECT count(*) AS mismatched, count(*) = 0 AS ok
FROM edu.student_assessments sa JOIN edu.assessments a ON a.id_assessment = sa.id_assessment
WHERE (sa.code_module, sa.code_presentation) <> (a.code_module, a.code_presentation);

\echo '5. Điểm trung bình theo kết quả cuối khóa so với CSV'
\echo '   Bỏ nhóm kỳ 2014J, Scotland, 0-35 ở cả hai phía: bước cài nguyên nhân ở edu đổi kết quả của nhóm này (câu 6),'
\echo '   nên câu này qua cả khi có và khi không có bước cài.'
SELECT db.final_result, db.avg_score, csv.avg_score AS csv_avg_score, db.avg_score = csv.avg_score AS ok
FROM (SELECT s.final_result, round(avg(sa.score), 4) AS avg_score
      FROM edu.student_assessments sa
      JOIN edu.students s ON (s.code_module, s.code_presentation, s.id_student) = (sa.code_module, sa.code_presentation, sa.id_student)
      WHERE NOT (s.code_presentation = '2014J' AND s.region = 'Scotland' AND s.age_band = '0-35')
      GROUP BY 1) db
JOIN (SELECT i.final_result, round(avg(nullif(sa.score, '?')::numeric), 4) AS avg_score
      FROM staging.oulad_student_assessment sa
      JOIN staging.oulad_assessments a ON a.id_assessment = sa.id_assessment
      JOIN staging.oulad_student_info i ON (i.code_module, i.code_presentation, i.id_student) = (a.code_module, a.code_presentation, sa.id_student)
      WHERE NOT (i.code_presentation = '2014J' AND i.region = 'Scotland' AND i.age_band = '0-35')
      GROUP BY 1) csv USING (final_result)
ORDER BY 1;

\echo '6. Nguyên nhân cài sẵn ở edu (sql/plant_demo_cause_edu.sql): đúng 381 sinh viên khác CSV, là mọi sinh viên'
\echo '   của nhóm đã định (kỳ 2014J, Scotland, nhóm tuổi 0-35, CSV ghi Pass), mỗi dòng chỉ đổi final_result thành'
\echo '   Withdrawn. Nạp với SKIP_DEMO_CAUSE=1 thì changed_students là 0 và ok là f.'
WITH compared AS (
    SELECT s.final_result, i.final_result AS csv_final_result,
           -- So mọi cột mô tả với CSV: bước cài chỉ được đổi final_result.
           (s.gender, s.region, s.highest_education, s.imd_band, s.age_band, s.num_of_prev_attempts,
            s.studied_credits)
               IS DISTINCT FROM (i.gender, i.region, i.highest_education,
                                 CASE WHEN i.imd_band IN ('?', '') THEN NULL
                                      WHEN i.imd_band = '10-20' THEN '10-20%' ELSE i.imd_band END,
                                 i.age_band, i.num_of_prev_attempts::integer, i.studied_credits::integer)
               AS other_columns_changed,
           s.final_result IS DISTINCT FROM i.final_result AS changed,
           (s.code_presentation = '2014J' AND s.region = 'Scotland' AND s.age_band = '0-35'
            AND i.final_result = 'Pass') AS in_target
    FROM edu.students s
    JOIN staging.oulad_student_info i
        ON (i.code_module, i.code_presentation, i.id_student::integer) = (s.code_module, s.code_presentation, s.id_student)
),
summary AS (
    SELECT count(*) FILTER (WHERE changed) AS changed_students,
           count(*) FILTER (WHERE in_target) AS target_students,
           count(*) FILTER (WHERE changed AND NOT (in_target AND final_result = 'Withdrawn')) AS wrong_changes,
           count(*) FILTER (WHERE in_target AND NOT changed) AS missed_students,
           count(*) FILTER (WHERE other_columns_changed) AS other_columns_changed
    FROM compared
)
SELECT changed_students, target_students, wrong_changes, missed_students, other_columns_changed,
       changed_students = 381 AND target_students = 381 AND wrong_changes = 0 AND missed_students = 0
           AND other_columns_changed = 0 AS ok
FROM summary;
