# docs/images/src/luong-cau-hoi.py - sinh sơ đồ luồng một câu hỏi (docs/images/luong-cau-hoi.png, ở README và Pages).
# Hàng trên đi tới DuckDB, hàng dưới quay về giao diện; Postgres, mô hình ngôn ngữ, DuckDB nằm giữa.
# Logo trong logos.json (đường SVG). Chạy: python3 docs/images/src/luong-cau-hoi.py, rồi chụp file HTML sinh ra bằng
# Chrome không giao diện:
#   google-chrome --headless=new --hide-scrollbars --force-device-scale-factor=2 --window-size=1600,1110 \
#     --virtual-time-budget=10000 --screenshot=docs/images/luong-cau-hoi.png docs/images/src/luong-cau-hoi.html
# Sửa chữ ở đây khi luồng trong code đổi (docs/ARCHITECTURE.md mục luồng một câu hỏi).
import json
import os
from html import escape

HERE = os.path.dirname(os.path.abspath(__file__))

LOGOS = json.load(open(os.path.join(HERE, "logos.json")))
W, H = 1600, 1110
INK, MUTED = "#13262f", "#5a6d72"
REQ, RET, NUM = "#33454c", "#2f7a73", "#d98a1e"
out = []


def text(x, y, lines, size=13, weight=400, fill=MUTED, anchor="middle", lh=None):
    lh = lh or size * 1.4
    spans = "".join(f'<tspan x="{x}" dy="{0 if i == 0 else lh}">{escape(l)}</tspan>' for i, l in enumerate(lines))
    out.append(f'<text x="{x}" y="{y}" font-size="{size}" font-weight="{weight}" fill="{fill}" text-anchor="{anchor}">{spans}</text>')


def logo(name, x, y, size):
    out.append(f'<g transform="translate({x},{y}) scale({size / 24})">{LOGOS[name]}</g>')


def rect(x, y, w, h, fill, stroke, dashed=False, rx=14):
    dash = ' stroke-dasharray="7 5"' if dashed else ""
    out.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="{fill}" stroke="{stroke}" stroke-width="1.6"{dash}/>')


def module(x, y, w, h, title, lines):
    rect(x, y, w, h, "#ffffff", "#e3a54a")
    top = y + h / 2 - len(lines) * 9 + 2
    text(x + w / 2, top, [title], size=15, weight=700, fill=INK)
    text(x + w / 2, top + 21, lines, size=12.5, lh=17)


def arrow(points, color=REQ, dashed=False, width=2):
    d = "M" + " L".join(f"{px} {py}" for px, py in points)
    dash = ' stroke-dasharray="7 5"' if dashed else ""
    out.append(f'<path d="{d}" fill="none" stroke="{color}" stroke-width="{width}"{dash} marker-end="url(#ah)"/>')


def num(x, y, n, color=NUM):
    out.append(f'<circle cx="{x}" cy="{y}" r="14" fill="{color}" stroke="#ffffff" stroke-width="2.5"/>')
    text(x, y + 5, [str(n)], size=13.5, weight=700, fill="#ffffff")


out.append(f'<rect width="{W}" height="{H}" fill="#ffffff"/>')
out.append(
    '<svg x="32" y="24" width="44" height="44" viewBox="0 0 32 32"><rect width="32" height="32" rx="8" fill="#d98a1e"/>'
    '<path d="M10 23v-9M16 23V9M22 23v-6" stroke="#13262f" stroke-width="3" stroke-linecap="round"/></svg>'
)
text(90, 46, ["Một câu hỏi đi qua xdi-platform như thế nào"], size=24, weight=700, fill=INK, anchor="start")
text(90, 70, ["Đi theo số: hàng trên đi tới DuckDB, hàng dưới quay về giao diện. Mũi tên đậm là lượt gọi, mũi tên xanh là kết quả trả về, nét đứt là đường phụ."], size=14, anchor="start")

# ---- khung backend
rect(388, 104, 970, 690, "#f3f7fd", "#a9c1e8", dashed=True, rx=20)
logo("rust", 404, 116, 26)
text(440, 135, ["xdi-backend (Rust, một binary)"], size=15, weight=700, fill=INK, anchor="start")

# ---- người dùng, giao diện
rect(28, 330, 150, 150, "#f1eefc", "#b9b0ec")
out.append('<circle cx="103" cy="376" r="17" fill="#7c6fd0"/><path d="M73 428 a30 30 0 0 1 60 0 z" fill="#7c6fd0"/>')
text(103, 458, ["Người dùng"], size=15, weight=700, fill=INK)

rect(210, 160, 150, 590, "#e6f7fb", "#9ad7e6")
logo("react", 255, 380, 60)
text(285, 476, ["Giao diện"], size=15, weight=700, fill=INK)
text(285, 497, ["React, Vite"], size=12.5)
text(285, 560, ["bảng, biểu đồ,", "lời giải thích,", "Minh chứng:", "SQL, các bước,", "phân rã,", "dòng nguồn"], size=12, lh=16)

# ---- hàng trên
TY, TH = 165, 100
cols = {"api": (408, 160), "ctx": (600, 220), "sql": (860, 220), "guard": (1120, 220)}
module(*cols["api"], TY, TH, "API", ["Axum: đăng nhập,", "quyền workspace"]) if False else None
x, w = cols["api"]; module(x, TY, w, TH, "API", ["Axum: đăng nhập,", "quyền workspace"])
x, w = cols["ctx"]; module(x, TY, w, TH, "Dựng ngữ cảnh", ["bảng của dataset, metadata", "cột, chỉ số, câu mẫu, bài học"])
x, w = cols["sql"]; module(x, TY, w, TH, "Hiểu câu hỏi", ["phân loại; viết SQL; câu \"vì sao\":", "chọn khuôn, điền form"])
x, w = cols["guard"]; module(x, TY, w, TH, "Guard SQL", ["một SELECT; JOIN theo khóa", "ngoại; lọc dòng theo vai; LIMIT"])

# ---- giữa: Postgres, mô hình, DuckDB, database công ty
MY, MH = 360, 190


def external(x, w, fill, stroke, logos, title, lines):
    rect(x, MY, w, MH, fill, stroke)
    lx = x + 18
    for name, size, dy in logos:
        logo(name, lx, MY + dy, size)
    text(x + 88, MY + 62, [title], size=16, weight=700, fill=INK, anchor="start")
    text(x + 88, MY + 86, lines, size=12.5, anchor="start", lh=17)


external(600, 220, "#e8f0fb", "#a9c1e8", [("postgres", 56, 40)], "Postgres", ["dataset, bảng,", "quan hệ, hồ sơ cột,", "chỉ số, câu mẫu,", "bài học, lịch sử,", "nhật ký lỗi"])
external(860, 220, "#fdf3e6", "#e9b97a", [("gemini", 40, 34), ("ollama", 40, 100)], "Mô hình", ["Gemini, Ollama, mock;", "nhận metadata cột,", "giá trị cột phân loại,", "các ý do code tính;", "không nhận dòng dữ liệu"])
external(1120, 220, "#fdf9e1", "#e2cf6a", [("duckdb", 52, 40)], "DuckDB nhúng", ["chạy SQL đã qua", "guard trên Parquet;", "số do DuckDB và", "code tính"])

rect(1390, MY, 186, MH, "#ffffff", "#9aa9ad", dashed=True)
text(1483, MY + 34, ["Trước khi hỏi"], size=12.5, weight=700, fill=NUM)
text(1483, MY + 62, ["Database công ty"], size=15, weight=700, fill=INK)
text(1483, MY + 86, ["Postgres, phiên chỉ đọc;", "DuckDB đọc thẳng, ghi", "Parquet; đọc khóa ngoại,", "COMMENT ON; SQL của AI", "không chạy trên đây"], size=12.5, lh=17)
arrow([(1390, MY + MH / 2), (1342, MY + MH / 2)], color="#9aa9ad", dashed=True)

# ---- hàng dưới
BY, BH = 640, 100
module(1120, BY, 220, BH, "Kết quả và giải thích", ["bảng, biểu đồ, \"Đã hiểu là\", cảnh", "báo; phân rã (SQL qua guard)"])
module(860, BY, 220, BH, "Viết lời, kiểm hai cổng", ["cổng 1: code so từng số;", "cổng 2: mô hình khác chấm nghĩa"])
module(600, BY, 220, BH, "Ghi lại", ["câu hỏi, SQL, các bước;", "lỗi vào nhật ký lỗi"])

# ---- đường chính
ty = TY + 45
arrow([(178, 392), (210, 392)]); num(194, 372, 1)
arrow([(360, ty), (408, ty)]); num(384, ty - 22, 2)
arrow([(568, ty), (600, ty)]); num(584, ty - 22, 3)
arrow([(820, ty), (860, ty)]); num(840, ty - 22, 5)
arrow([(1080, ty), (1120, ty)]); num(1100, ty - 22, 7)

# ngữ cảnh <-> Postgres
arrow([(698, TY + TH), (698, MY)]); arrow([(722, MY), (722, TY + TH)], color=RET); num(670, 312, 4)
# hiểu câu hỏi <-> mô hình
arrow([(958, TY + TH), (958, MY)]); arrow([(982, MY), (982, TY + TH)], color=RET); num(930, 312, 6)
# guard -> DuckDB
arrow([(1230, TY + TH), (1230, MY)]); num(1258, 312, 8)
# guard sai -> mô hình sửa một lần
arrow([(1136, TY + TH), (1136, 300), (1100, 300), (1100, MY + 22), (1082, MY + 22)], color=NUM, dashed=True)
text(1094, 308, ["từ chối, lỗi,", "lọc sai: viết", "lại một lần"], size=11.5, fill=NUM, anchor="end", lh=15)
# DuckDB -> kết quả (và SQL phân rã đi lên)
arrow([(1218, MY + MH), (1218, BY)], color=RET); arrow([(1242, BY), (1242, MY + MH)]); num(1270, 595, 9)
text(1252, 620, ["phân rã"], size=12, fill=MUTED, anchor="start")
# kết quả -> viết lời <-> mô hình
arrow([(1120, BY + 50), (1080, BY + 50)], color=RET); num(1100, BY + 28, 10)
arrow([(982, BY), (982, MY + MH)]); arrow([(958, MY + MH), (958, BY)], color=RET); num(930, 595, 11)
# viết lời -> ghi lại -> Postgres
arrow([(860, BY + 50), (820, BY + 50)], color=RET); num(840, BY + 28, 12)
arrow([(710, BY), (710, MY + MH)]); num(682, 595, 13)
# ghi lại -> giao diện -> người dùng
arrow([(600, BY + 50), (360, BY + 50)], color=RET); num(480, BY + 28, 14)
arrow([(210, 440), (178, 440)], color=RET); num(194, 464, 15)
# phản hồi: Tốt / Chưa đạt -> câu mẫu, bài học
arrow([(285, 750), (285, 830), (660, 830), (660, BY + BH + 4)], color=RET, dashed=True)
num(470, 830, 16, color=RET)
text(300, 856, ["Tốt: admin duyệt thành câu mẫu. Chưa đạt: vào nhật ký lỗi, admin viết bài học. Dùng ở bước 4 câu sau."], size=12, fill=RET, anchor="start")
# ghi lại cũng ghi phản hồi vào Postgres: mũi tên lên đã có ở bước 13

# ---- chú giải các bước
steps = [
    "Người dùng chọn dataset (một database hay một file) và gõ câu hỏi.",
    "Giao diện gửi câu hỏi tới API; API kiểm đăng nhập và quyền workspace.",
    "API chuyển câu hỏi cho bước dựng ngữ cảnh.",
    "Chọn bảng và các bảng nối theo khóa ngoại; đọc metadata cột, giá trị, chỉ số, câu mẫu, bài học.",
    "Một cụm khớp nhiều cột thì chọn cột hợp nhất, ghi các cột còn lại thành giả định đổi được.",
    "Mô hình phân loại, nêu cột cần, viết SQL; câu \"vì sao\": chọn khuôn, điền form, code dựng SQL.",
    "Guard: một SELECT, bảng, cột, hàm được phép, JOIN đúng khóa ngoại, lọc dòng theo vai, LIMIT.",
    "Chặn cộng lặp; DuckDB chạy trên Parquet; bị từ chối, lỗi hay lọc sai giá trị thì mô hình viết lại một lần.",
    "Bảng kết quả, cảnh báo; code tính phần giải thích, phân rã bằng SQL do code dựng (cũng qua guard).",
    "Các ý do code tính (không có dòng dữ liệu) đi tới bước viết lời.",
    "Mô hình viết lời; code so từng số, mô hình khác chấm nghĩa; sai thì dùng câu của code.",
    "Câu trả lời đã kiểm đi tới bước ghi lại.",
    "Ghi câu hỏi, SQL, các bước vào lịch sử; lỗi của mô hình vào nhật ký lỗi.",
    "Backend trả một gói JSON cho giao diện.",
    "Người dùng xem bảng, biểu đồ, lời giải thích, tab phân rã; bấm vào số để xem dòng nguồn.",
    "Tốt: admin duyệt thành câu mẫu. Chưa đạt: vào nhật ký lỗi, admin viết bài học.",
]
LY = 870
rect(28, LY, 1548, 222, "#f7f9f9", "#dde4e6", rx=16)
for i, s in enumerate(steps):
    col, r = divmod(i, 8)
    x = 52 + col * 774
    y = LY + 30 + r * 24.5
    num(x, y - 4, i + 1, color=RET if i == 15 else NUM)
    text(x + 24, y + 1, [s], size=13, fill=INK, anchor="start")

svg = "\n".join(o for o in out if o)
html = f"""<!doctype html><html lang="vi"><head><meta charset="utf-8"><title>xdi-platform: luồng một câu hỏi</title>
<link href="https://fonts.googleapis.com/css2?family=Be+Vietnam+Pro:wght@400;500;600;700&display=swap" rel="stylesheet">
<style>body{{margin:0;background:#fff}} svg{{display:block}}</style></head><body>
<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Luồng một câu hỏi qua xdi-platform" viewBox="0 0 {W} {H}" width="{W}" height="{H}" font-family="Be Vietnam Pro, system-ui, sans-serif">
<defs><marker id="ah" viewBox="0 0 10 10" refX="8.5" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0 L10 5 L0 10z" fill="context-stroke"/></marker></defs>
{svg}
</svg></body></html>"""
open(os.path.join(HERE, "luong-cau-hoi.html"), "w").write(html)
