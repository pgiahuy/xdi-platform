# data/postgres-demo/admin/app.py - màn xem nhanh hai cơ sở dữ liệu mẫu sales_demo (schema sales) và edu_demo (schema
# edu) bằng Flask-Admin, để khi trình diễn đối chiếu số ứng dụng trả lời với dòng dữ liệu gốc của "công ty". Mỗi
# database một nhóm trên menu. Chỉ đọc hai lớp: kết nối bằng người dùng analyst (chỉ SELECT, giao dịch mặc định chỉ
# đọc), và mọi view tắt thêm, sửa, xóa. Không phụ thuộc vào backend của ứng dụng; cấu hình trong docker-compose.yml
# cùng thư mục cha (service admin, profile admin).

import logging
import os

from flask import Flask, redirect, url_for
from flask_admin import Admin, AdminIndexView, expose
from flask_admin.contrib.sqla import ModelView
from flask_admin.theme import Bootstrap4Theme
from flask_babel import Babel
from sqlalchemy import MetaData, String, Text, create_engine, func, select, text
from sqlalchemy.exc import SQLAlchemyError
from sqlalchemy.ext.automap import automap_base
from sqlalchemy.orm import scoped_session, sessionmaker

# Máy chủ không kèm tên database, và danh sách database cách nhau bằng dấu phẩy.
SERVER_URL = os.environ.get("DEMO_SERVER_URL", "postgresql+psycopg://analyst:analyst@127.0.0.1:5440")
DATABASES = [d.strip() for d in os.environ.get("DEMO_DATABASES", "sales_demo,edu_demo").split(",") if d.strip()]
PAGE_SIZE = 50

# Schema mà analyst đọc được, trừ schema hệ thống. Lọc theo quyền nên schema staging (load.sh với KEEP_STAGING=1)
# không hiện: analyst không có quyền trên nó, hiện ra thì mọi trang của nó báo lỗi quyền.
READABLE_SCHEMAS = text(
    "SELECT nspname FROM pg_namespace "
    "WHERE has_schema_privilege(nspname, 'USAGE') AND nspname NOT LIKE 'pg\\_%' AND nspname <> 'information_schema' "
    "ORDER BY nspname"
)

log = logging.getLogger(__name__)


class DemoDatabase:
    """Một database mẫu: engine, session và bảng đọc từ chính database (cột, khóa, COMMENT ON)."""

    def __init__(self, name):
        self.name = name
        self.engine = create_engine(f"{SERVER_URL}/{name}", pool_pre_ping=True)
        self.session = scoped_session(sessionmaker(bind=self.engine))
        self.metadata = MetaData()
        self.error = None
        try:
            with self.engine.connect() as conn:
                schemas = list(conn.execute(READABLE_SCHEMAS).scalars())
            for schema in schemas:
                self.metadata.reflect(bind=self.engine, schema=schema)
        except SQLAlchemyError as e:
            # Database chưa nạp (load.sh) hay chưa chạy: vẫn mở được màn, trang tổng quan ghi lỗi của database này.
            log.warning("cannot read database %s: %s", name, e)
            self.error = str(e.__cause__ or e).splitlines()[0]
        self.base = automap_base(metadata=self.metadata)
        # Tên lớp kèm schema để hai schema có bảng cùng tên không đè nhau. Bỏ relationship tự sinh vì khóa ngoại đã
        # hiện ở dạng cột, và tên relationship tự sinh có thể trùng nhau.
        self.base.prepare(
            classname_for_table=lambda base, tablename, table: f"{table.schema}_{tablename}",
            generate_relationship=lambda *args, **kwargs: None,
        )

    def tables(self):
        return sorted(self.metadata.tables.values(), key=lambda t: (t.schema, t.name))

    def model(self, table):
        return self.base.classes[f"{table.schema}_{table.name}"]

    def endpoint(self, table):
        return f"{self.name}_{table.schema}_{table.name}"


def count_rows(db, table):
    with db.engine.connect() as conn:
        return conn.execute(select(func.count()).select_from(table)).scalar_one()


def describe_foreign_keys(table):
    """Mỗi khóa ngoại thành một chuỗi "cột -> bảng (cột)"."""
    lines = []
    for fk in table.foreign_key_constraints:
        target = fk.referred_table
        source_cols = ", ".join(c.name for c in fk.columns)
        target_cols = ", ".join(e.column.name for e in fk.elements)
        lines.append(f"{source_cols} -> {target.name} ({target_cols})")
    return lines


class ReadOnlyView(ModelView):
    can_create = False
    can_edit = False
    can_delete = False
    can_view_details = True
    can_set_page_size = True
    column_display_pk = True
    page_size = PAGE_SIZE

    def __init__(self, model, table, session, **kwargs):
        columns = list(table.columns)
        self.column_list = [c.name for c in columns]
        self.column_details_list = self.column_list
        self.column_sortable_list = self.column_list
        self.column_filters = self.column_list
        self.column_searchable_list = [c.name for c in columns if isinstance(c.type, (String, Text))]
        # Giữ đúng tên cột như trong database (Flask-Admin mặc định viết thành "Code Module"), để khớp tên cột mà
        # ứng dụng và câu SQL dùng.
        self.column_labels = {c.name: c.name for c in columns}
        # Mô tả tiếng Việt của cột (COMMENT ON COLUMN) hiện khi rê chuột lên tên cột.
        self.column_descriptions = {c.name: c.comment for c in columns if c.comment}
        super().__init__(model, session, **kwargs)


class HomeView(AdminIndexView):
    def __init__(self, databases, **kwargs):
        self.databases = databases
        super().__init__(**kwargs)

    @expose("/")
    def index(self):
        groups = []
        for db in self.databases:
            tables = [
                {
                    "name": f"{table.schema}.{table.name}",
                    "comment": table.comment or "",
                    "rows": count_rows(db, table),
                    "columns": len(table.columns),
                    "primary_key": ", ".join(c.name for c in table.primary_key.columns),
                    "foreign_keys": describe_foreign_keys(table),
                    "endpoint": f"{db.endpoint(table)}.index_view",
                }
                for table in db.tables()
            ]
            groups.append({"database": db.name, "error": db.error, "tables": tables})
        return self.render("home.html", groups=groups)


def create_app():
    app = Flask(__name__)
    app.config["SECRET_KEY"] = os.environ.get("ADMIN_SECRET_KEY", os.urandom(24).hex())
    app.config["BABEL_DEFAULT_LOCALE"] = "vi"
    Babel(app)

    databases = [DemoDatabase(name) for name in DATABASES]
    admin = Admin(
        app,
        name="Dữ liệu mẫu",
        index_view=HomeView(databases, name="Tổng quan", url="/"),
        theme=Bootstrap4Theme(swatch="flatly"),
    )
    for db in databases:
        for table in db.tables():
            admin.add_view(
                ReadOnlyView(
                    db.model(table),
                    table,
                    db.session,
                    name=table.name,
                    category=db.name,
                    endpoint=db.endpoint(table),
                )
            )

    @app.teardown_appcontext
    def remove_sessions(exception=None):
        for db in databases:
            db.session.remove()

    @app.route("/admin")
    def old_index():
        return redirect(url_for("admin.index"))

    return app


app = create_app()

if __name__ == "__main__":
    app.run(host="127.0.0.1", port=int(os.environ.get("ADMIN_PORT", "8091")))
