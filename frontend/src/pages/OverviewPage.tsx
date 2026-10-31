// SPDX-License-Identifier: Apache-2.0
/* pages/OverviewPage.tsx - trang "Tổng quan dữ liệu" của workspace (/w/:ws/overview), mục riêng trên thanh bên: chọn
một dataset (database hay file, #257) ở ô Dataset đầu trang, xem danh sách bảng của nó (TableIndex.tsx), bấm một bảng
thì xem tổng quan của bảng đó (DatasetOverviewSection.tsx, khung tổng quan theo bảng có sẵn). File có một bảng nên chỉ
có khung tổng quan.
Dataset và bảng đang xem nằm trong đường dẫn (?dataset=...&table=..., askScope.ts), nên nút Tổng quan của một bảng ở
trang Dữ liệu mở thẳng đúng bảng và tải lại trang vẫn giữ; link cũ ghi id bảng vào ?dataset= vẫn mở đúng bảng. Chưa
ghi bảng thì xem bảng đầu tiên của dataset. */

import { Link, useParams, useSearchParams } from "react-router";
import { useCatalog } from "@/api/catalog";
import { DatasetScopePicker } from "@/components/ask/DatasetScopePicker";
import { EmptyState } from "@/components/EmptyState";
import { TableIndex } from "@/components/overview/TableIndex";
import { Button } from "@/components/ui/button";
import { buildScopeOptions, resolveAskScope } from "./askScope";
import { DatasetOverviewSection } from "./DatasetOverviewSection";
import { workspaceHref } from "./workspaceNav";

export function OverviewPage() {
  const { ws = "" } = useParams();
  const datasets = useCatalog(ws);
  const [params, setParams] = useSearchParams();
  const { dataset, table: pinned } = resolveAskScope(
    datasets.data ?? [],
    params.get("dataset"),
    params.get("table"),
  );
  const table = pinned ?? dataset?.tables[0];

  if (datasets.error && !datasets.data) {
    return (
      <EmptyState
        title="Không tải được danh sách dataset"
        description={datasets.error.message}
        action={
          <Button variant="outline" onClick={() => datasets.refetch()}>
            Thử lại
          </Button>
        }
      />
    );
  }
  if (!datasets.data)
    return <p className="text-sm text-muted-foreground">Đang tải...</p>;
  if (!dataset) {
    return (
      <EmptyState
        title="Chưa có dataset nào"
        description="Dataset nạp ở trang Dữ liệu sẽ có tổng quan ở đây."
        action={
          <Button variant="outline" asChild>
            <Link to={workspaceHref(ws, "datasets")}>Tới trang Dữ liệu</Link>
          </Button>
        }
      />
    );
  }
  const showIndex = dataset.kind === "database" && dataset.tables.length > 0;
  return (
    // Bung hết bề ngang trang: biểu đồ cần chỗ, màn rộng không để trống một khoảng lớn bên phải.
    <div className="flex flex-col gap-4">
      <DatasetScopePicker
        datasets={buildScopeOptions(datasets.data)}
        datasetId={dataset.id}
        onChange={(id) => setParams({ dataset: id })}
        // Bảng chọn ở danh sách bên dưới, không ở ô chọn.
        pinTables={false}
        // Khung tổng quan tự ghi phần hồ sơ chưa có (lời của backend), nên dưới ô chọn không báo lại lần nữa.
        profile="ready"
        disabled={false}
      />
      {dataset.tables.length === 0 && (
        <p className="text-sm text-muted-foreground">
          Database này đã kết nối nhưng chưa chụp bảng nào; chụp bảng ở trang Dữ liệu.
        </p>
      )}
      <div className={showIndex ? "grid items-start gap-4 lg:grid-cols-[18rem_minmax(0,1fr)]" : "flex flex-col"}>
        {showIndex && (
          <TableIndex
            tables={dataset.tables.map((t) => ({
              id: t.id,
              name: t.name,
              schema: t.schema,
              rowCount: t.row_count,
              description: t.description,
            }))}
            selectedId={table?.id}
            onSelect={(id) => setParams({ dataset: dataset.id, table: id })}
          />
        )}
        {table && (
          <DatasetOverviewSection
            key={table.id}
            ws={ws}
            datasetId={dataset.id}
            table={{ id: table.id, name: dataset.kind === "file" ? dataset.name : table.name }}
          />
        )}
      </div>
    </div>
  );
}
