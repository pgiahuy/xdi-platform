# data/postgres-demo/admin/Dockerfile - image của màn xem nhanh sales_demo và edu_demo, chạy bằng gunicorn.
FROM python:3.12-slim
WORKDIR /app
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt
COPY app.py .
COPY templates templates
# Người dùng không phải root. Cần thư mục home vì gunicorn mở socket điều khiển ở đó.
RUN useradd --system --create-home admin
USER admin
EXPOSE 8091
CMD ["gunicorn", "--bind", "0.0.0.0:8091", "--workers", "2", "app:app"]
