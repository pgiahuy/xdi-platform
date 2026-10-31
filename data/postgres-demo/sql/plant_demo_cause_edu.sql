-- sql/plant_demo_cause_edu.sql - cố ý cài MỘT nguyên nhân biết trước vào dữ liệu học tập, để thử câu hỏi tìm nguyên
-- nhân với đáp án đã biết. Đây là dữ liệu sửa tay, không phải dữ liệu OULAD gốc.
--
-- load.sh chạy file này sau khi đã đổ dữ liệu thật (sql/edu_load.sql), trong cùng giao dịch, cạnh
-- sql/plant_demo_cause_sales.sql. Bỏ qua cả hai bằng SKIP_DEMO_CAUSE=1 sh load.sh: khi đó edu giữ đúng như CSV.
--
-- Nguyên nhân cài vào: kỳ 2014J, mọi sinh viên vùng Scotland, nhóm tuổi 0-35, có kết quả Pass, thành Withdrawn.
-- Chỉ đổi một cột, edu.students.final_result. Không đổi ngày hủy đăng ký, điểm hay lượt bấm: những sinh viên này
-- vẫn có điểm thi và lượt bấm tới cuối kỳ, và edu.registrations.date_unregistration vẫn rỗng. README.md ghi rõ
-- dấu vết này. Nhóm được chọn vì là nhóm (vùng, nhóm tuổi) đông nhất của kỳ 2014J (925 sinh viên) và trước khi sửa
-- có tỷ lệ bỏ học thấp hơn trung bình kỳ, nên sau khi sửa nó nổi lên rõ khi so kỳ 2014J với 2014B. Sinh viên của
-- nhóm học cả 7 môn, nên chia theo môn không ra đáp án. Đáp án mong đợi: Scotland và nhóm tuổi 0-35. README.md (mục
-- "Nguyên nhân cài sẵn") ghi số sinh viên và tỷ lệ trước, sau.
--
-- Cố ý không ghi gì về việc sửa này vào COMMENT của bảng hay cột: ứng dụng có thể đưa mô tả cột cho mô hình ngôn
-- ngữ, ghi vào đó là lộ đáp án.

CREATE TEMP TABLE demo_cause_students ON COMMIT DROP AS
SELECT code_module, code_presentation, id_student
FROM edu.students
WHERE code_presentation = '2014J'
  AND region = 'Scotland'
  AND age_band = '0-35'
  AND final_result = 'Pass';

-- In tỷ lệ trước khi đổi, để người chạy load.sh thấy ngay mức thay đổi.
SELECT 'before' AS stage,
       count(*) FILTER (WHERE region = 'Scotland' AND age_band = '0-35') AS slice_students,
       round(100.0 * avg((final_result = 'Withdrawn')::integer) FILTER (WHERE region = 'Scotland' AND age_band = '0-35'), 1) AS slice_withdrawn_pct,
       count(*) AS term_students,
       round(100.0 * avg((final_result = 'Withdrawn')::integer), 1) AS term_withdrawn_pct
FROM edu.students
WHERE code_presentation = '2014J';

UPDATE edu.students s
SET final_result = 'Withdrawn'
FROM demo_cause_students d
WHERE (s.code_module, s.code_presentation, s.id_student) = (d.code_module, d.code_presentation, d.id_student);

SELECT 'after' AS stage,
       count(*) FILTER (WHERE region = 'Scotland' AND age_band = '0-35') AS slice_students,
       round(100.0 * avg((final_result = 'Withdrawn')::integer) FILTER (WHERE region = 'Scotland' AND age_band = '0-35'), 1) AS slice_withdrawn_pct,
       count(*) AS term_students,
       round(100.0 * avg((final_result = 'Withdrawn')::integer), 1) AS term_withdrawn_pct,
       (SELECT count(*) FROM demo_cause_students) AS changed_students
FROM edu.students
WHERE code_presentation = '2014J';
