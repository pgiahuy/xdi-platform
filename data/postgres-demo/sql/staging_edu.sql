-- sql/staging_edu.sql - schema "staging" của edu_demo: mỗi file CSV của OULAD một bảng, mọi cột kiểu text, đúng
-- thứ tự cột trong file. load.sh chép CSV vào đây bằng \copy, rồi sql/edu_load.sql đổi kiểu và tách sang bảng chuẩn
-- hóa. Để mọi cột là text: COPY không dừng giữa chừng vì một giá trị lạ ("?", chuỗi rỗng), việc đổi kiểu nằm trong
-- SQL đọc được. Người dùng analyst không có quyền trên schema này. DataCo có staging riêng trong sales_demo
-- (sql/staging_sales.sql).

DROP SCHEMA IF EXISTS staging CASCADE;
CREATE SCHEMA staging;

CREATE TABLE staging.oulad_courses (
    code_module text,
    code_presentation text,
    module_presentation_length text
);

CREATE TABLE staging.oulad_assessments (
    code_module text,
    code_presentation text,
    id_assessment text,
    assessment_type text,
    date text,
    weight text
);

CREATE TABLE staging.oulad_student_info (
    code_module text,
    code_presentation text,
    id_student text,
    gender text,
    region text,
    highest_education text,
    imd_band text,
    age_band text,
    num_of_prev_attempts text,
    studied_credits text,
    disability text,
    final_result text
);

CREATE TABLE staging.oulad_student_assessment (
    id_assessment text,
    id_student text,
    date_submitted text,
    is_banked text,
    score text
);

CREATE TABLE staging.oulad_student_registration (
    code_module text,
    code_presentation text,
    id_student text,
    date_registration text,
    date_unregistration text
);

CREATE TABLE staging.oulad_vle (
    id_site text,
    code_module text,
    code_presentation text,
    activity_type text,
    week_from text,
    week_to text
);

CREATE TABLE staging.oulad_student_vle (
    code_module text,
    code_presentation text,
    id_student text,
    id_site text,
    date text,
    sum_click text
);
