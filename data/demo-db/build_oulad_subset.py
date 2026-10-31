#!/usr/bin/env python3
# data/demo-db/build_oulad_subset.py - dựng bản nhỏ của OULAD trong data/samples/oulad/ từ bản đầy đủ đã tải bằng
# `sh data/download.sh oulad`. Database mẫu của compose (data/demo-db/README.md) nạp bản nhỏ này, để bản cài mới không
# phải tải 464 MB.
#
# Cách chạy, từ gốc repo: python3 data/demo-db/build_oulad_subset.py [thư mục OULAD] [thư mục ra]
# Mặc định đọc data/downloads/oulad, ghi data/samples/oulad. Chỉ dùng thư viện chuẩn. Chạy lại ra đúng cùng file:
# sinh viên chọn bằng random.Random với hạt giống cố định.
#
# Cách chọn: mỗi học phần (môn, kỳ) lấy STUDENTS_PER_COURSE sinh viên ngẫu nhiên; giữ mọi dòng của các sinh viên đó
# trong studentRegistration.csv và studentAssessment.csv (chỉ bài của đúng học phần, để khóa ngoại tới edu.students
# đúng). courses.csv, assessments.csv, vle.csv giữ mọi dòng. studentVle.csv (454 MB) không chép: gộp sẵn theo (sinh
# viên, ngày) thành studentDailyClicks.csv, đúng cách sql/edu_load.sql của data/postgres-demo gộp bản đầy đủ.

import csv
import random
import sys
from collections import defaultdict
from pathlib import Path

STUDENTS_PER_COURSE = 30
SEED = 230


def read_rows(path):
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def write_rows(path, fields, rows):
    with open(path, "w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def main():
    src = Path(sys.argv[1] if len(sys.argv) > 1 else "data/downloads/oulad")
    out = Path(sys.argv[2] if len(sys.argv) > 2 else "data/samples/oulad")
    out.mkdir(parents=True, exist_ok=True)

    # Chép mọi dòng, ghi lại bằng module csv để mọi file cùng kiểu xuống dòng LF (file gốc dùng CRLF).
    for name in ("courses.csv", "assessments.csv", "vle.csv"):
        rows = read_rows(src / name)
        write_rows(out / name, list(rows[0].keys()), rows)

    info = read_rows(src / "studentInfo.csv")
    by_course = defaultdict(list)
    for row in info:
        by_course[(row["code_module"], row["code_presentation"])].append(row)
    rng = random.Random(SEED)
    chosen = set()
    for course in sorted(by_course):
        rows = sorted(by_course[course], key=lambda r: int(r["id_student"]))
        for row in rng.sample(rows, min(STUDENTS_PER_COURSE, len(rows))):
            chosen.add((row["code_module"], row["code_presentation"], row["id_student"]))
    # Giữ thứ tự dòng của file gốc.
    kept_info = [r for r in info if (r["code_module"], r["code_presentation"], r["id_student"]) in chosen]
    write_rows(out / "studentInfo.csv", list(info[0].keys()), kept_info)

    reg = read_rows(src / "studentRegistration.csv")
    kept_reg = [r for r in reg if (r["code_module"], r["code_presentation"], r["id_student"]) in chosen]
    write_rows(out / "studentRegistration.csv", list(reg[0].keys()), kept_reg)

    course_of = {
        r["id_assessment"]: (r["code_module"], r["code_presentation"]) for r in read_rows(src / "assessments.csv")
    }
    sa = read_rows(src / "studentAssessment.csv")
    kept_sa = [r for r in sa if course_of[r["id_assessment"]] + (r["id_student"],) in chosen]
    write_rows(out / "studentAssessment.csv", list(sa[0].keys()), kept_sa)

    # Đọc từng dòng, không nạp cả file 454 MB vào RAM.
    clicks = defaultdict(int)
    sites = defaultdict(set)
    with open(src / "studentVle.csv", newline="", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            key = (row["code_module"], row["code_presentation"], row["id_student"])
            if key not in chosen:
                continue
            day = key + (int(row["date"]),)
            clicks[day] += int(row["sum_click"])
            sites[day].add(row["id_site"])
    daily = [
        {
            "code_module": d[0],
            "code_presentation": d[1],
            "id_student": d[2],
            "date": d[3],
            "sum_click": clicks[d],
            "num_sites": len(sites[d]),
        }
        for d in sorted(clicks, key=lambda d: (d[0], d[1], int(d[2]), d[3]))
    ]
    write_rows(
        out / "studentDailyClicks.csv",
        ["code_module", "code_presentation", "id_student", "date", "sum_click", "num_sites"],
        daily,
    )
    print(f"{len(kept_info)} students, {len(kept_reg)} registrations, {len(kept_sa)} assessment results, "
          f"{len(daily)} student days")


if __name__ == "__main__":
    main()
