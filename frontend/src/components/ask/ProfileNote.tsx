// SPDX-License-Identifier: Apache-2.0
/* components/ask/ProfileNote.tsx - một dòng dưới chỗ chọn dataset, báo hồ sơ của dataset đang chọn chưa xong, không có
câu gợi ý với mô hình hiện tại, hoặc lỗi.
Xong rồi thì không hiện gì: câu gợi ý hiện ra là đủ biết. Không có dòng này thì người vừa nạp file thấy trang hỏi không
có câu gợi ý và không biết là chờ thêm hay dataset có vấn đề. Lời báo lỗi theo lời backend trả khi hỏi dataset lỗi hồ
sơ (service/ask/context.rs): phiên bản đó không hỏi được, cần nạp lại file. no_suggestions không phải lỗi (hỏi vẫn
được), nên dùng màu chữ phụ như dòng đang lập, không dùng màu lỗi. */

import type { ProfileProgress } from './types'

export function ProfileNote({ progress }: { progress: ProfileProgress }) {
  if (progress === 'profiling') {
    return (
      <p role="status" className="text-xs text-muted-foreground">
        Đang lập hồ sơ dataset: câu gợi ý và mô tả cột sẽ có sau ít phút.
      </p>
    )
  }
  if (progress === 'no_suggestions') {
    return (
      <p role="status" className="text-xs text-muted-foreground">
        Mô hình hiện tại không lập được câu gợi ý cho dataset này; vẫn hỏi được bình thường.
      </p>
    )
  }
  if (progress === 'failed') {
    return (
      <p role="alert" className="text-xs text-destructive">
        Lập hồ sơ dataset lỗi nên chưa hỏi được phiên bản này; nạp lại file ở trang Dữ liệu.
      </p>
    )
  }
  return null
}
