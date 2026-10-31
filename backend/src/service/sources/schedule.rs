// SPDX-License-Identifier: Apache-2.0
/* service/sources/schedule.rs - chụp lại mọi nguồn Postgres theo lịch (SOURCE_RESNAPSHOT_HOURS, #247). main.rs chạy
run_resnapshot_schedule trong một task nền; không đặt biến (hay đặt 0) thì không có lịch, admin bấm "Chụp lại".

Mỗi vòng chụp lại từng nguồn bằng đúng hàm của route POST /sources/{id}/resnapshot, nhân danh một admin của workspace
(auth::scheduled_source_admins): bảng không đổi không tạo phiên bản mới, bảng quá trần giữ metadata. Các nguồn chụp lần
lượt, không song song, để database nguồn và đĩa tạm chỉ chịu một lần chụp mỗi lúc. Vòng đầu chạy sau một khoảng lịch,
không chạy lúc khởi động: khởi động lại nhiều lần không thành nhiều lần chụp. Lỗi của một nguồn chỉ ghi log mức cảnh
báo, nguồn khác vẫn chụp. */

use crate::auth::scheduled_source_admins;
use crate::state::AppState;

pub async fn run_resnapshot_schedule(state: AppState) {
    let Some(every) = state.config.source_resnapshot_every else {
        return;
    };
    tracing::info!(
        hours = every.as_secs() / 3600,
        "scheduled source re-snapshot is on"
    );
    let mut ticker = tokio::time::interval_at(tokio::time::Instant::now() + every, every);
    // Một vòng chụp kéo dài hơn khoảng lịch thì vòng sau chạy ngay khi xong, không dồn nhiều vòng bù.
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticker.tick().await;
        resnapshot_all(&state).await;
    }
}

async fn resnapshot_all(state: &AppState) {
    let sources = match scheduled_source_admins(&state.store).await {
        Ok(sources) => sources,
        Err(e) => {
            tracing::warn!(error = ?e, "cannot list sources for the scheduled re-snapshot");
            return;
        }
    };
    for (member, source_id) in sources {
        match super::resnapshot(state, &member, source_id).await {
            Ok(done) => tracing::info!(
                %source_id,
                tables = done.datasets.len(),
                new_versions = done.datasets.iter().filter(|d| d.created).count(),
                metadata_only = done.metadata_only.len(),
                unreadable = done.unreadable.len(),
                "scheduled source re-snapshot done"
            ),
            Err(e) => tracing::warn!(%source_id, error = ?e, "scheduled source re-snapshot failed"),
        }
    }
}
