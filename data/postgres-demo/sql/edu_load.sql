-- sql/edu_load.sql - đổ dữ liệu OULAD từ schema staging (mọi cột text, xem sql/staging.sql) sang schema edu.
-- load.sh chạy file này sau sql/edu_schema.sql, trong cùng một giao dịch: lỗi ở bất kỳ dòng nào thì không bảng nào
-- đổi. Thứ tự INSERT theo khóa ngoại: bảng được tham chiếu nạp trước.
--
-- OULAD ghi giá trị thiếu là "?". Hàm edu_null đổi "?" và chuỗi rỗng thành NULL trước khi đổi kiểu; không làm vậy
-- thì "?"::integer báo lỗi và cả lần nạp dừng.

CREATE FUNCTION pg_temp.edu_null(value text) RETURNS text
    LANGUAGE sql IMMUTABLE
    RETURN nullif(nullif(btrim(value), '?'), '');

INSERT INTO edu.courses (code_module, code_presentation, presentation_year, presentation_start_month,
                         module_presentation_length)
SELECT code_module,
       code_presentation,
       left(code_presentation, 4)::integer,
       CASE right(code_presentation, 1) WHEN 'B' THEN 2 WHEN 'J' THEN 10 END,
       module_presentation_length::integer
FROM staging.oulad_courses;

INSERT INTO edu.students (code_module, code_presentation, id_student, gender, region, highest_education, imd_band,
                          age_band, num_of_prev_attempts, studied_credits, disability, final_result)
SELECT code_module,
       code_presentation,
       id_student::integer,
       gender,
       region,
       highest_education,
       -- Nguồn ghi "10-20" thiếu dấu %, các nhóm khác đều có; thêm vào để nhóm theo imd_band ra đúng 10 nhóm.
       CASE pg_temp.edu_null(imd_band) WHEN '10-20' THEN '10-20%' ELSE pg_temp.edu_null(imd_band) END,
       age_band,
       num_of_prev_attempts::integer,
       studied_credits::integer,
       disability = 'Y',
       final_result
FROM staging.oulad_student_info;

INSERT INTO edu.registrations (code_module, code_presentation, id_student, date_registration, date_unregistration)
SELECT code_module,
       code_presentation,
       id_student::integer,
       pg_temp.edu_null(date_registration)::integer,
       pg_temp.edu_null(date_unregistration)::integer
FROM staging.oulad_student_registration;

INSERT INTO edu.assessments (id_assessment, code_module, code_presentation, assessment_type, date, weight)
SELECT id_assessment::integer,
       code_module,
       code_presentation,
       assessment_type,
       pg_temp.edu_null(date)::integer,
       weight::numeric
FROM staging.oulad_assessments;

-- Môn và kỳ lấy từ bài đánh giá, để khóa ngoại tới edu.students kiểm được sinh viên có học đúng học phần đó.
INSERT INTO edu.student_assessments (id_assessment, id_student, code_module, code_presentation, date_submitted,
                                     is_banked, score)
SELECT sa.id_assessment::integer,
       sa.id_student::integer,
       a.code_module,
       a.code_presentation,
       sa.date_submitted::integer,
       sa.is_banked = '1',
       pg_temp.edu_null(sa.score)::numeric
FROM staging.oulad_student_assessment sa
JOIN staging.oulad_assessments a ON a.id_assessment = sa.id_assessment;

INSERT INTO edu.vle_materials (id_site, code_module, code_presentation, activity_type, week_from, week_to)
SELECT id_site::integer,
       code_module,
       code_presentation,
       activity_type,
       pg_temp.edu_null(week_from)::integer,
       pg_temp.edu_null(week_to)::integer
FROM staging.oulad_vle;

-- Gộp 10,6 triệu dòng studentVle theo sinh viên và ngày, lý do ở sql/edu_schema.sql.
INSERT INTO edu.student_daily_clicks (code_module, code_presentation, id_student, date, sum_click, num_sites)
SELECT code_module,
       code_presentation,
       id_student::integer,
       date::integer,
       sum(sum_click::integer),
       count(DISTINCT id_site)
FROM staging.oulad_student_vle
GROUP BY code_module, code_presentation, id_student::integer, date::integer;
