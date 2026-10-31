-- sql/row_counts.sql - đếm dòng thật (count(*), không phải ước lượng của planner) của từng bảng trong một schema;
-- load.sh in ra sau khi nạp mỗi database, với biến psql schema (psql -v schema=sales).
SELECT table_schema || '.' || table_name AS table_name,
       (xpath('/row/n/text()',
              query_to_xml(format('SELECT count(*) AS n FROM %I.%I', table_schema, table_name), false, true, '')))[1]::text::bigint
           AS row_count
FROM information_schema.tables
WHERE table_schema = :'schema' AND table_type = 'BASE TABLE'
ORDER BY table_name;
