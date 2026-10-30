# Bản dịch tiếng Việt của prompt

Mỗi file ở đây là bản dịch của file cùng tên trong `backend/prompts/`, để cả đội đọc và góp ý. Chỉ để đọc: backend
không nhúng và không gửi các file này cho mô hình; mô hình luôn nhận bản tiếng Anh (luật viết tiếng Anh vì là phần máy
đọc, xem AGENTS.md mục 1).

Sửa một prompt tiếng Anh thì sửa bản dịch cùng lúc. Test `vietnamese_mirrors_match_the_prompts` (`engine/prompts.rs`)
báo lỗi khi một prompt chưa có bản dịch, hay khi hai bản khác nhau về các chỗ trống `{{...}}`. Test không kiểm được
nghĩa của câu dịch.
