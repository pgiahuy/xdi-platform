-- sql/sales_schema.sql - schema "sales" của database sales_demo: dữ liệu bán hàng DataCo, từ một file phẳng một dòng
-- mỗi dòng hàng, tách thành bảng phòng ban, danh mục, sản phẩm, khách hàng, đơn hàng và dòng hàng (bảng sự kiện).
-- load.sh chạy file này mỗi lần nạp: xóa schema cũ rồi tạo lại. Dữ liệu đổ vào trong sql/sales_load.sql.
--
-- Cột nào thuộc bảng nào đã kiểm bằng phụ thuộc hàm trên cả 180 519 dòng (README.md, mục "Tách bảng DataCo"):
-- mọi cột của khách hàng chỉ có một giá trị cho mỗi Customer Id, mọi cột của đơn hàng chỉ có một giá trị cho mỗi
-- Order Id, và tương tự cho sản phẩm, danh mục, phòng ban. Tiền, số lượng, giảm giá, lợi nhuận đổi theo từng dòng
-- hàng nên nằm ở sales.order_items.
--
-- Số tiền để kiểu numeric không giới hạn số lẻ: giữ đúng giá trị trong CSV (có số như 314.6400146), để tổng trên
-- Postgres khớp tổng tính trên file gốc. Làm tròn về 2 số lẻ thì tổng lệch vài đơn vị.

DROP SCHEMA IF EXISTS sales CASCADE;
CREATE SCHEMA sales;
COMMENT ON SCHEMA sales IS 'Dữ liệu bán hàng và giao hàng DataCo: phòng ban, danh mục, sản phẩm, khách hàng, đơn hàng, dòng hàng';

CREATE TABLE sales.departments (
    department_id integer PRIMARY KEY,
    department_name text NOT NULL
);
COMMENT ON TABLE sales.departments IS 'Phòng ban (nhóm hàng lớn) của cửa hàng';
COMMENT ON COLUMN sales.departments.department_id IS 'Mã phòng ban (Department Id)';
COMMENT ON COLUMN sales.departments.department_name IS 'Tên phòng ban, ví dụ Fitness, Apparel (Department Name)';

CREATE TABLE sales.categories (
    category_id integer PRIMARY KEY,
    category_name text NOT NULL,
    department_id integer NOT NULL REFERENCES sales.departments (department_id)
);
COMMENT ON TABLE sales.categories IS 'Danh mục sản phẩm; mỗi danh mục thuộc một phòng ban';
COMMENT ON COLUMN sales.categories.category_id IS 'Mã danh mục (Category Id, trùng Product Category Id)';
COMMENT ON COLUMN sales.categories.category_name IS 'Tên danh mục, ví dụ Cleats, Fishing (Category Name)';
COMMENT ON COLUMN sales.categories.department_id IS 'Phòng ban chứa danh mục, khóa ngoại tới sales.departments';

CREATE TABLE sales.products (
    product_id integer PRIMARY KEY,
    product_name text NOT NULL,
    category_id integer NOT NULL REFERENCES sales.categories (category_id),
    price numeric NOT NULL
);
COMMENT ON TABLE sales.products IS 'Sản phẩm; mỗi sản phẩm thuộc một danh mục';
COMMENT ON COLUMN sales.products.product_id IS 'Mã sản phẩm (Product Card Id, trùng Order Item Cardprod Id)';
COMMENT ON COLUMN sales.products.product_name IS 'Tên sản phẩm (Product Name)';
COMMENT ON COLUMN sales.products.category_id IS 'Danh mục của sản phẩm, khóa ngoại tới sales.categories';
COMMENT ON COLUMN sales.products.price IS 'Giá niêm yết của sản phẩm, đô la Mỹ (Product Price)';

CREATE TABLE sales.customers (
    customer_id integer PRIMARY KEY,
    first_name text NOT NULL,
    last_name text,
    segment text NOT NULL,
    street text,
    city text NOT NULL,
    state text NOT NULL,
    country text NOT NULL,
    zipcode text,
    latitude numeric NOT NULL,
    longitude numeric NOT NULL
);
COMMENT ON TABLE sales.customers IS 'Khách hàng. Email và mật khẩu trong nguồn đã bị che (cùng một giá trị XXXXXXXXX) nên không đưa vào';
COMMENT ON COLUMN sales.customers.customer_id IS 'Mã khách hàng (Customer Id, trùng Order Customer Id)';
COMMENT ON COLUMN sales.customers.first_name IS 'Tên (Customer Fname)';
COMMENT ON COLUMN sales.customers.last_name IS 'Họ (Customer Lname). Nguồn để trống ở 8 dòng';
COMMENT ON COLUMN sales.customers.segment IS 'Phân khúc khách: Consumer (cá nhân), Corporate (doanh nghiệp), Home Office (văn phòng tại nhà)';
COMMENT ON COLUMN sales.customers.street IS 'Địa chỉ đường phố của khách. Trống ở 3 khách bị lệch cột trong nguồn (xem sql/sales_load.sql)';
COMMENT ON COLUMN sales.customers.city IS 'Thành phố nơi khách ở';
COMMENT ON COLUMN sales.customers.state IS 'Bang nơi khách ở, mã hai chữ, ví dụ CA, PR';
COMMENT ON COLUMN sales.customers.country IS 'Nước nơi khách ở, như trong nguồn: EE. UU. (Hoa Kỳ) hoặc Puerto Rico';
COMMENT ON COLUMN sales.customers.zipcode IS 'Mã bưu chính, như trong nguồn (số 0 ở đầu đã mất, ví dụ 725 là 00725). Không trống ở khách nào sau khi dời 3 khách bị lệch cột';
COMMENT ON COLUMN sales.customers.latitude IS 'Vĩ độ. Mô tả của DataCo gọi là vị trí cửa hàng, nhưng trong dữ liệu mỗi khách chỉ có một giá trị';
COMMENT ON COLUMN sales.customers.longitude IS 'Kinh độ, cùng ghi chú với latitude';

CREATE TABLE sales.orders (
    order_id integer PRIMARY KEY,
    customer_id integer NOT NULL REFERENCES sales.customers (customer_id),
    order_date timestamp NOT NULL,
    shipping_date timestamp NOT NULL,
    payment_type text NOT NULL,
    order_status text NOT NULL,
    delivery_status text NOT NULL,
    late_delivery_risk boolean NOT NULL,
    days_for_shipping_real integer NOT NULL,
    days_for_shipment_scheduled integer NOT NULL,
    shipping_mode text NOT NULL,
    market text NOT NULL,
    order_region text NOT NULL,
    order_country text NOT NULL,
    order_state text NOT NULL,
    order_city text NOT NULL,
    order_zipcode text
);
COMMENT ON TABLE sales.orders IS 'Đơn hàng: ai đặt, khi nào, giao đi đâu, giao thế nào. Một đơn có một hoặc nhiều dòng hàng';
COMMENT ON COLUMN sales.orders.order_id IS 'Mã đơn hàng (Order Id)';
COMMENT ON COLUMN sales.orders.customer_id IS 'Khách đặt đơn, khóa ngoại tới sales.customers';
COMMENT ON COLUMN sales.orders.order_date IS 'Thời điểm đặt hàng (order date (DateOrders))';
COMMENT ON COLUMN sales.orders.shipping_date IS 'Thời điểm giao hàng (shipping date (DateOrders))';
COMMENT ON COLUMN sales.orders.payment_type IS 'Hình thức thanh toán: DEBIT, TRANSFER, CASH, PAYMENT (cột Type trong nguồn)';
COMMENT ON COLUMN sales.orders.order_status IS 'Trạng thái đơn, ví dụ COMPLETE, PENDING, CANCELED, SUSPECTED_FRAUD';
COMMENT ON COLUMN sales.orders.delivery_status IS 'Tình trạng giao: Advance shipping (sớm), Shipping on time (đúng hạn), Late delivery (trễ), Shipping canceled (hủy)';
COMMENT ON COLUMN sales.orders.late_delivery_risk IS 'Giao trễ hay không: true đúng với các đơn có delivery_status = Late delivery (Late_delivery_risk = 1). Đơn hủy (Shipping canceled) luôn là false, kể cả khi số ngày giao thực tế vượt lịch';
COMMENT ON COLUMN sales.orders.days_for_shipping_real IS 'Số ngày giao thực tế';
COMMENT ON COLUMN sales.orders.days_for_shipment_scheduled IS 'Số ngày giao theo lịch';
COMMENT ON COLUMN sales.orders.shipping_mode IS 'Cách giao: Standard Class, Second Class, First Class, Same Day';
COMMENT ON COLUMN sales.orders.market IS 'Thị trường nơi giao hàng: Africa, Europe, LATAM, Pacific Asia, USCA';
COMMENT ON COLUMN sales.orders.order_region IS 'Vùng nơi giao hàng, ví dụ Southeast Asia, Western Europe';
COMMENT ON COLUMN sales.orders.order_country IS 'Nước nơi giao hàng, tên tiếng Tây Ban Nha như trong nguồn';
COMMENT ON COLUMN sales.orders.order_state IS 'Bang hoặc tỉnh nơi giao hàng';
COMMENT ON COLUMN sales.orders.order_city IS 'Thành phố nơi giao hàng';
COMMENT ON COLUMN sales.orders.order_zipcode IS 'Mã bưu chính nơi giao hàng. Nguồn chỉ có ở một phần đơn tại Hoa Kỳ, còn lại trống';

CREATE TABLE sales.order_items (
    order_item_id integer PRIMARY KEY,
    order_id integer NOT NULL REFERENCES sales.orders (order_id),
    product_id integer NOT NULL REFERENCES sales.products (product_id),
    quantity integer NOT NULL,
    unit_price numeric NOT NULL,
    sales numeric NOT NULL,
    discount numeric NOT NULL,
    discount_rate numeric NOT NULL,
    item_total numeric NOT NULL,
    profit numeric NOT NULL,
    profit_ratio numeric NOT NULL
);
COMMENT ON TABLE sales.order_items IS 'Dòng hàng (bảng sự kiện): một sản phẩm trong một đơn, kèm số lượng, tiền, giảm giá, lợi nhuận. Mỗi dòng của file DataCo là một dòng ở đây';
COMMENT ON COLUMN sales.order_items.order_item_id IS 'Mã dòng hàng (Order Item Id)';
COMMENT ON COLUMN sales.order_items.order_id IS 'Đơn chứa dòng hàng, khóa ngoại tới sales.orders';
COMMENT ON COLUMN sales.order_items.product_id IS 'Sản phẩm bán, khóa ngoại tới sales.products';
COMMENT ON COLUMN sales.order_items.quantity IS 'Số lượng (Order Item Quantity)';
COMMENT ON COLUMN sales.order_items.unit_price IS 'Đơn giá lúc bán, đô la Mỹ (Order Item Product Price). Trong dữ liệu luôn bằng sales.products.price';
COMMENT ON COLUMN sales.order_items.sales IS 'Doanh số trước giảm giá, bằng đơn giá nhân số lượng (Sales)';
COMMENT ON COLUMN sales.order_items.discount IS 'Số tiền giảm giá (Order Item Discount)';
COMMENT ON COLUMN sales.order_items.discount_rate IS 'Tỷ lệ giảm giá, từ 0 đến 1 (Order Item Discount Rate)';
COMMENT ON COLUMN sales.order_items.item_total IS 'Thành tiền sau giảm giá (Order Item Total; cột Sales per customer trong nguồn có cùng giá trị ở mọi dòng)';
COMMENT ON COLUMN sales.order_items.profit IS 'Lợi nhuận của dòng hàng, có thể âm (Order Profit Per Order; cột Benefit per order có cùng giá trị ở mọi dòng). Tên nguồn nói "per order" nhưng giá trị đổi theo từng dòng hàng';
COMMENT ON COLUMN sales.order_items.profit_ratio IS 'Tỷ suất lợi nhuận, bằng profit chia item_total (Order Item Profit Ratio)';
