// SPDX-License-Identifier: Apache-2.0
/* components/ErrorBoundary.tsx - chặn lỗi của một phần trang để nó không làm mất cả trang. React chỉ bắt được lỗi khi
vẽ bằng class component (chưa có hook tương đương), nên đây là class component duy nhất của giao diện.
Không có ranh giới này, lỗi đi lên errorElement của route gốc (router.tsx) và cả trang chỉ còn màn báo lỗi. Dùng cho
phần tải riêng có thể hỏng mà phần còn lại vẫn dùng được, ví dụ biểu đồ: tab mở từ trước một lần cập nhật xin file
chunk không còn trên máy chủ, mạng đứt đúng lúc tải, hoặc thư viện ném lỗi với dữ liệu lạ. */

import { Component, type ReactNode } from 'react'

type Props = {
  /** Hiện thay cho children khi children ném lỗi. */
  fallback: ReactNode
  children: ReactNode
}

type State = {
  failed: boolean
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { failed: false }

  static getDerivedStateFromError(): State {
    return { failed: true }
  }

  render() {
    return this.state.failed ? this.props.fallback : this.props.children
  }
}
