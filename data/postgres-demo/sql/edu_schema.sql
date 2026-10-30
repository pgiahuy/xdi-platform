-- sql/edu_schema.sql - schema "edu": dữ liệu học tập OULAD (Open University, Anh) tách thành bảng có khóa.
-- load.sh chạy file này mỗi lần nạp: xóa schema cũ rồi tạo lại, nên đổi bảng ở đây là đủ, không cần migration.
-- Dữ liệu đổ vào từ schema staging trong sql/edu_load.sql. Quan hệ giữa các bảng vẽ trong README.md.
--
-- Một "học phần" (course) là một môn (code_module) mở trong một kỳ (code_presentation). Mã sinh viên id_student
-- không duy nhất một mình: một người học nhiều môn hoặc học lại thì có nhiều dòng trong students.

DROP SCHEMA IF EXISTS edu CASCADE;
CREATE SCHEMA edu;
COMMENT ON SCHEMA edu IS 'Dữ liệu học tập OULAD: học phần, sinh viên, bài đánh giá, điểm, tài liệu học trực tuyến';

CREATE TABLE edu.courses (
    code_module text NOT NULL,
    code_presentation text NOT NULL,
    presentation_year integer NOT NULL,
    presentation_start_month integer NOT NULL CHECK (presentation_start_month IN (2, 10)),
    module_presentation_length integer NOT NULL,
    PRIMARY KEY (code_module, code_presentation)
);
COMMENT ON TABLE edu.courses IS 'Học phần: một môn mở trong một kỳ. Mỗi dòng một cặp (môn, kỳ)';
COMMENT ON COLUMN edu.courses.code_module IS 'Mã môn học, ví dụ AAA';
COMMENT ON COLUMN edu.courses.code_presentation IS 'Mã kỳ mở môn: năm và chữ B (bắt đầu tháng 2) hoặc J (bắt đầu tháng 10), ví dụ 2013J';
COMMENT ON COLUMN edu.courses.presentation_year IS 'Năm bắt đầu kỳ, tách từ code_presentation';
COMMENT ON COLUMN edu.courses.presentation_start_month IS 'Tháng bắt đầu kỳ: 2 với chữ B, 10 với chữ J, tách từ code_presentation';
COMMENT ON COLUMN edu.courses.module_presentation_length IS 'Độ dài học phần, tính bằng ngày';

CREATE TABLE edu.students (
    code_module text NOT NULL,
    code_presentation text NOT NULL,
    id_student integer NOT NULL,
    gender text NOT NULL CHECK (gender IN ('M', 'F')),
    region text NOT NULL,
    highest_education text NOT NULL,
    imd_band text,
    age_band text NOT NULL,
    num_of_prev_attempts integer NOT NULL,
    studied_credits integer NOT NULL,
    disability boolean NOT NULL,
    final_result text NOT NULL CHECK (final_result IN ('Distinction', 'Pass', 'Fail', 'Withdrawn')),
    PRIMARY KEY (code_module, code_presentation, id_student),
    FOREIGN KEY (code_module, code_presentation) REFERENCES edu.courses (code_module, code_presentation)
);
COMMENT ON TABLE edu.students IS 'Sinh viên trong một học phần (từ studentInfo.csv). Mỗi dòng một sinh viên học một môn trong một kỳ';
COMMENT ON COLUMN edu.students.code_module IS 'Mã môn học, cùng edu.courses';
COMMENT ON COLUMN edu.students.code_presentation IS 'Mã kỳ, cùng edu.courses';
COMMENT ON COLUMN edu.students.id_student IS 'Mã sinh viên (đã ẩn danh). Một sinh viên có thể có nhiều dòng nếu học nhiều học phần';
COMMENT ON COLUMN edu.students.gender IS 'Giới tính: M nam, F nữ';
COMMENT ON COLUMN edu.students.region IS 'Vùng địa lý nơi sinh viên sống khi học';
COMMENT ON COLUMN edu.students.highest_education IS 'Trình độ học vấn cao nhất khi vào học phần';
COMMENT ON COLUMN edu.students.imd_band IS 'Nhóm chỉ số thiếu thốn (Index of Multiple Deprivation) của nơi ở, theo khoảng phần trăm; 0-10% là thiếu thốn nhất. Rỗng khi nguồn ghi "?"';
COMMENT ON COLUMN edu.students.age_band IS 'Nhóm tuổi: 0-35, 35-55, 55<=';
COMMENT ON COLUMN edu.students.num_of_prev_attempts IS 'Số lần đã học môn này trước đó';
COMMENT ON COLUMN edu.students.studied_credits IS 'Tổng số tín chỉ sinh viên đang học trong kỳ';
COMMENT ON COLUMN edu.students.disability IS 'Sinh viên có khai báo khuyết tật hay không';
COMMENT ON COLUMN edu.students.final_result IS 'Kết quả cuối học phần: Distinction (giỏi), Pass (đạt), Fail (trượt), Withdrawn (bỏ học)';

CREATE TABLE edu.registrations (
    code_module text NOT NULL,
    code_presentation text NOT NULL,
    id_student integer NOT NULL,
    date_registration integer,
    date_unregistration integer,
    PRIMARY KEY (code_module, code_presentation, id_student),
    FOREIGN KEY (code_module, code_presentation, id_student)
        REFERENCES edu.students (code_module, code_presentation, id_student)
);
COMMENT ON TABLE edu.registrations IS 'Ngày đăng ký và ngày hủy đăng ký học phần của sinh viên (từ studentRegistration.csv). Mỗi dòng ứng với đúng một dòng edu.students';
COMMENT ON COLUMN edu.registrations.code_module IS 'Mã môn học';
COMMENT ON COLUMN edu.registrations.code_presentation IS 'Mã kỳ';
COMMENT ON COLUMN edu.registrations.id_student IS 'Mã sinh viên';
COMMENT ON COLUMN edu.registrations.date_registration IS 'Ngày đăng ký, tính bằng số ngày so với ngày bắt đầu học phần (âm là đăng ký trước). Rỗng khi nguồn ghi "?"';
COMMENT ON COLUMN edu.registrations.date_unregistration IS 'Ngày hủy đăng ký, cùng cách tính. Rỗng nếu sinh viên không hủy';

CREATE TABLE edu.assessments (
    id_assessment integer PRIMARY KEY,
    code_module text NOT NULL,
    code_presentation text NOT NULL,
    assessment_type text NOT NULL CHECK (assessment_type IN ('TMA', 'CMA', 'Exam')),
    date integer,
    weight numeric NOT NULL,
    FOREIGN KEY (code_module, code_presentation) REFERENCES edu.courses (code_module, code_presentation)
);
COMMENT ON TABLE edu.assessments IS 'Bài đánh giá của một học phần: bài tập chấm bởi giảng viên, bài trắc nghiệm máy chấm, bài thi cuối kỳ';
COMMENT ON COLUMN edu.assessments.id_assessment IS 'Mã bài đánh giá';
COMMENT ON COLUMN edu.assessments.code_module IS 'Mã môn học';
COMMENT ON COLUMN edu.assessments.code_presentation IS 'Mã kỳ';
COMMENT ON COLUMN edu.assessments.assessment_type IS 'Loại bài: TMA (bài tập giảng viên chấm), CMA (bài máy chấm), Exam (thi cuối kỳ)';
COMMENT ON COLUMN edu.assessments.date IS 'Hạn nộp, tính bằng số ngày từ ngày bắt đầu học phần. Rỗng khi nguồn ghi "?" (bài thi chưa có ngày, theo mô tả OULAD là cuối học phần)';
COMMENT ON COLUMN edu.assessments.weight IS 'Trọng số của bài, phần trăm. Trong một học phần, các bài không phải thi cộng lại 100 (môn GGG: 0) và bài thi cộng lại 100 (môn CCC có hai bài thi, cộng lại 200)';

CREATE TABLE edu.student_assessments (
    id_assessment integer NOT NULL REFERENCES edu.assessments (id_assessment),
    id_student integer NOT NULL,
    code_module text NOT NULL,
    code_presentation text NOT NULL,
    date_submitted integer NOT NULL,
    is_banked boolean NOT NULL,
    score numeric CHECK (score BETWEEN 0 AND 100),
    PRIMARY KEY (id_assessment, id_student),
    FOREIGN KEY (code_module, code_presentation, id_student)
        REFERENCES edu.students (code_module, code_presentation, id_student)
);
COMMENT ON TABLE edu.student_assessments IS 'Điểm từng bài đánh giá của sinh viên (từ studentAssessment.csv). Mỗi dòng một sinh viên nộp một bài';
COMMENT ON COLUMN edu.student_assessments.id_assessment IS 'Mã bài đánh giá, khóa ngoại tới edu.assessments';
COMMENT ON COLUMN edu.student_assessments.id_student IS 'Mã sinh viên';
COMMENT ON COLUMN edu.student_assessments.code_module IS 'Mã môn của bài đánh giá, chép từ edu.assessments để có khóa ngoại tới edu.students';
COMMENT ON COLUMN edu.student_assessments.code_presentation IS 'Mã kỳ của bài đánh giá, chép từ edu.assessments để có khóa ngoại tới edu.students';
COMMENT ON COLUMN edu.student_assessments.date_submitted IS 'Ngày nộp, tính bằng số ngày từ ngày bắt đầu học phần';
COMMENT ON COLUMN edu.student_assessments.is_banked IS 'Điểm chuyển từ lần học trước (true) hay làm trong lần này (false)';
COMMENT ON COLUMN edu.student_assessments.score IS 'Điểm từ 0 đến 100; dưới 40 là trượt bài. Rỗng khi nguồn ghi "?"';

CREATE TABLE edu.vle_materials (
    id_site integer PRIMARY KEY,
    code_module text NOT NULL,
    code_presentation text NOT NULL,
    activity_type text NOT NULL,
    week_from integer,
    week_to integer,
    FOREIGN KEY (code_module, code_presentation) REFERENCES edu.courses (code_module, code_presentation)
);
COMMENT ON TABLE edu.vle_materials IS 'Tài liệu và hoạt động trên hệ thống học trực tuyến (VLE) của một học phần (từ vle.csv)';
COMMENT ON COLUMN edu.vle_materials.id_site IS 'Mã tài liệu';
COMMENT ON COLUMN edu.vle_materials.code_module IS 'Mã môn học';
COMMENT ON COLUMN edu.vle_materials.code_presentation IS 'Mã kỳ';
COMMENT ON COLUMN edu.vle_materials.activity_type IS 'Loại hoạt động, ví dụ resource (tài liệu), forumng (diễn đàn), quiz (trắc nghiệm), oucontent (nội dung bài học)';
COMMENT ON COLUMN edu.vle_materials.week_from IS 'Tuần dự kiến bắt đầu dùng tài liệu. Rỗng khi nguồn ghi "?"';
COMMENT ON COLUMN edu.vle_materials.week_to IS 'Tuần dự kiến thôi dùng tài liệu. Rỗng khi nguồn ghi "?"';

-- studentVle.csv có 10,6 triệu dòng (mỗi lượt sinh viên mở một tài liệu trong một ngày, có khi lặp). Gộp theo
-- (sinh viên, tài liệu, ngày) vẫn còn 8,5 triệu dòng, quá nặng cho một bảng demo mà ứng dụng chụp về. Gộp theo
-- (sinh viên, ngày) còn 1,8 triệu dòng, giữ được trục thời gian để hỏi mức hoạt động theo tuần hay so với kết quả
-- cuối khóa; mất chi tiết từng tài liệu.
CREATE TABLE edu.student_daily_clicks (
    code_module text NOT NULL,
    code_presentation text NOT NULL,
    id_student integer NOT NULL,
    date integer NOT NULL,
    sum_click integer NOT NULL,
    num_sites integer NOT NULL,
    PRIMARY KEY (code_module, code_presentation, id_student, date),
    FOREIGN KEY (code_module, code_presentation, id_student)
        REFERENCES edu.students (code_module, code_presentation, id_student)
);
COMMENT ON TABLE edu.student_daily_clicks IS 'Số lần bấm trên hệ thống học trực tuyến của sinh viên theo ngày, gộp từ studentVle.csv. Mỗi dòng một sinh viên trong một học phần trong một ngày có hoạt động';
COMMENT ON COLUMN edu.student_daily_clicks.code_module IS 'Mã môn học';
COMMENT ON COLUMN edu.student_daily_clicks.code_presentation IS 'Mã kỳ';
COMMENT ON COLUMN edu.student_daily_clicks.id_student IS 'Mã sinh viên';
COMMENT ON COLUMN edu.student_daily_clicks.date IS 'Ngày, tính bằng số ngày từ ngày bắt đầu học phần (âm là trước khi bắt đầu)';
COMMENT ON COLUMN edu.student_daily_clicks.sum_click IS 'Tổng số lần bấm trong ngày, cộng trên mọi tài liệu';
COMMENT ON COLUMN edu.student_daily_clicks.num_sites IS 'Số tài liệu khác nhau sinh viên mở trong ngày';
