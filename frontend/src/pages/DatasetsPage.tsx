// SPDX-License-Identifier: Apache-2.0
/* pages/DatasetsPage.tsx - trang Dữ liệu của workspace: khung kết nối database (DatabaseSources.tsx, luồng chính, #230),
danh sách dataset, và nạp file là lựa chọn phụ cho admin: một nút mở khung nạp (DatasetUpload.tsx). Bấm "Nạp phiên bản
mới" ở một dataset nạp từ file thì khung nạp tự mở.
Một dataset là một database đã kết nối hay một file (#257, api/catalog.ts): mỗi dataset một thẻ (DatasetCard.tsx);
thẻ database mở ra thấy bảng nhóm theo schema và quan hệ, "Chụp lại" chụp lại mọi bảng của database trong một giao
dịch. Máy chủ của database lấy từ danh sách nguồn (chỉ admin đọc được; id dataset là id nguồn).
Vai lấy từ /auth/me (đã có trong bộ đệm, AppLayout vừa đọc). Ẩn khung nạp với member chỉ để giao diện gọn;
quyền thật nằm ở backend, member gọi thẳng API vẫn nhận 403.
Hồ sơ cột, chỉ số và biểu đồ dựng sẵn của từng bảng ở trang Tổng quan dữ liệu (OverviewPage.tsx, #198); nút Tổng
quan của một bảng mở trang đó với đúng bảng. */

import { useEffect, useRef, useState } from "react";
import { useParams } from "react-router";
import { useMe } from "@/api/auth";
import {
  useCatalog,
  useSnapshotCatalog,
  type CatalogDataset,
  type CatalogSnapshotResponse,
} from "@/api/catalog";
import { useDeleteDataset } from "@/api/datasets";
import { useSources, type Source } from "@/api/sources";
import { DatasetCard, type DatasetCardProps } from "@/components/datasets/DatasetCard";
import type { DatasetCardView, TableRow } from "@/components/datasets/types";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { DatabaseSources } from "./DatabaseSources";
import { DatasetUpload } from "./DatasetUpload";
import type { UploadTarget } from "./datasetUploadPlan";
import { RouterLink } from "./RouterLink";
import { overviewHref } from "./workspaceNav";
import { cn } from "@/lib/utils";

export function DatasetsPage() {
  const { ws = "" } = useParams();
  // Đổi workspace vẫn giữ trang này (cùng route), nên state cũ (file đã chọn, kết quả nạp, dataset đang nhận phiên
  // bản mới) sẽ đi theo sang workspace kia. key theo workspace để dựng lại từ đầu.
  return <WorkspaceDatasets key={ws} ws={ws} />;
}

type CardMessage = { id: string; kind: "error" | "success"; text: string };

function WorkspaceDatasets({ ws }: { ws: string }) {
  const me = useMe();
  const datasets = useCatalog(ws);
  const deleteTable = useDeleteDataset(ws);
  const snapshot = useSnapshotCatalog(ws);
  const [message, setMessage] = useState<CardMessage>();
  const [target, setTarget] = useState<UploadTarget>();
  const [uploadOpen, setUploadOpen] = useState(false);
  const uploadHeading = useRef<HTMLHeadingElement>(null);

  const isAdmin =
    me.data?.workspaces.find((m) => m.slug === ws)?.role === "admin";
  const sources = useSources(ws, isAdmin);
  // Khung chỉ đóng khi người dùng bấm đóng: nạp phiên bản mới xong (target về undefined) hay bỏ chọn dataset vẫn giữ
  // khung, để kết quả nạp và file đã chọn còn đó.
  const showUpload = uploadOpen;

  // Khung nạp có thể vừa mới mở (chưa có trong trang lúc bấm) hoặc đã cuộn khuất; sau khi vẽ thì đưa tiêu điểm lên đó
  // để người dùng thấy ngay mình đang nạp cho dataset nào, và người dùng bàn phím không phải lần ngược lên.
  useEffect(() => {
    if (!target) return;
    uploadHeading.current?.focus();
    uploadHeading.current?.scrollIntoView({ block: "start" });
  }, [target]);

  // File có một bảng; phiên bản mới nạp vào bảng đó, tên hiện ra là tên dataset.
  const addVersion = (dataset: CatalogDataset, table: TableRow) => {
    setTarget({ id: table.id, name: dataset.name });
    setUploadOpen(true);
  };

  const snapshotAgain = (dataset: CatalogDataset) => {
    snapshot.mutate(dataset.id, {
      onError: (error) => setMessage({ id: dataset.id, kind: "error", text: error.message }),
      onSuccess: (result) => setMessage({ id: dataset.id, kind: "success", text: snapshotText(result) }),
    });
  };

  const remove = (dataset: CatalogDataset, table: TableRow) => {
    deleteTable.mutate(table.id, {
      onError: (error) => setMessage({ id: dataset.id, kind: "error", text: error.message }),
      onSuccess: () => setMessage(undefined),
    });
  };

  const cardProps = (dataset: CatalogDataset): DatasetCardProps => {
    const source = sources.data?.find((s) => s.id === dataset.id);
    const shown = message?.id === dataset.id ? message : undefined;
    return {
      dataset: buildCardView(dataset, source),
      link: RouterLink,
      overviewHref: (table) => overviewHref(ws, dataset.id, table.id),
      metricsHref: (table) => `${table.id}/metrics`,
      onSnapshot: isAdmin ? () => snapshotAgain(dataset) : undefined,
      snapshotPending: snapshot.isPending && snapshot.variables === dataset.id,
      onAddVersion: isAdmin ? (table) => addVersion(dataset, table) : undefined,
      onDeleteTable: isAdmin ? (table) => remove(dataset, table) : undefined,
      deletePending: deleteTable.isPending,
      message: shown && { kind: shown.kind, text: shown.text },
    };
  };

  return (
    // Màn rộng (xl) hai cột: bên trái kết nối database và nạp file của admin, bên phải danh sách dataset; trang bung hết
    // bề ngang thay vì một cột hẹp chừa trống bên phải. Thành viên không có phần bên trái nên danh sách chiếm cả hàng.
    <div className={cn("grid items-start gap-8", isAdmin && "xl:grid-cols-2")}>
      {isAdmin && (
        <div className="flex min-w-0 flex-col gap-8">
          <DatabaseSources ws={ws} />
          {showUpload && (
            <div className="flex flex-col gap-2">
              <DatasetUpload
                ws={ws}
                target={target}
                onClearTarget={(id) =>
                  setTarget((current) =>
                    id === undefined || current?.id === id
                      ? undefined
                      : current,
                  )
                }
                headingRef={uploadHeading}
              />
              {!target && (
                <Button
                  variant="ghost"
                  size="sm"
                  className="self-start"
                  onClick={() => setUploadOpen(false)}
                >
                  Đóng khung nạp file
                </Button>
              )}
            </div>
          )}
          {!showUpload && (
            <div className="flex flex-wrap items-center gap-3 text-sm text-muted-foreground">
              <span>Dữ liệu chưa nằm trong database?</span>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setUploadOpen(true)}
              >
                Nạp file CSV, Parquet hoặc Excel
              </Button>
            </div>
          )}
        </div>
      )}

      <section
        aria-labelledby="dataset-list-heading"
        className="flex min-w-0 flex-col gap-3"
      >
        <h2
          id="dataset-list-heading"
          className="text-[15px] font-semibold tracking-tight"
        >
          Dataset của workspace
        </h2>
        <ListBody
          datasets={datasets.data}
          error={datasets.error}
          onRetry={() => datasets.refetch()}
          isAdmin={isAdmin}
          cardProps={cardProps}
        />
      </section>
    </div>
  );
}

type ListBodyProps = {
  datasets: CatalogDataset[] | undefined;
  error: Error | null;
  onRetry: () => void;
  isAdmin: boolean;
  cardProps: (dataset: CatalogDataset) => DatasetCardProps;
};

function ListBody({ datasets, error, onRetry, isAdmin, cardProps }: ListBodyProps) {
  if (error && !datasets) {
    return (
      <EmptyState
        title="Không tải được danh sách dataset"
        description={error.message}
        action={
          <Button variant="outline" onClick={onRetry}>
            Thử lại
          </Button>
        }
      />
    );
  }
  if (!datasets)
    return <p className="text-sm text-muted-foreground">Đang tải...</p>;
  if (datasets.length === 0) {
    return (
      <EmptyState
        title="Chưa có dataset nào"
        description={
          isAdmin
            ? "Kết nối database ở khung phía trên rồi chụp bảng, hoặc nạp file CSV, Parquet, Excel."
            : "Admin của workspace nạp dữ liệu; khi có, dataset hiện ở đây."
        }
      />
    );
  }
  return (
    <ul className="flex flex-col gap-3">
      {datasets.map((d) => (
        <DatasetCard key={d.id} {...cardProps(d)} />
      ))}
    </ul>
  );
}

function buildCardView(dataset: CatalogDataset, source: Source | undefined): DatasetCardView {
  const names = new Map(dataset.tables.map((t) => [t.id, t.name]));
  return {
    id: dataset.id,
    name: dataset.name,
    kind: dataset.kind,
    connection: source ? `${source.host}:${source.port}/${source.database}` : undefined,
    updatedAt: dataset.updated_at,
    tables: dataset.tables.map((t) => ({
      id: t.id,
      name: t.name,
      schema: t.schema,
      rowCount: t.row_count,
      description: t.description,
    })),
    relations: dataset.relations.map((r) => ({
      key: r.constraint_name + r.from_dataset_id,
      from: names.get(r.from_dataset_id) ?? r.from_dataset_name,
      fromColumns: r.from_columns,
      to: names.get(r.to_dataset_id) ?? r.to_dataset_name,
      toColumns: r.to_columns,
    })),
  };
}

function snapshotText(result: CatalogSnapshotResponse): string {
  const changed = result.datasets.filter((d) => d.created).length;
  const total = result.datasets.length;
  if (changed === 0) return `Đã chụp lại ${total} bảng, không bảng nào đổi.`;
  return `Đã chụp lại ${total} bảng, ${changed} bảng có phiên bản mới.`;
}
