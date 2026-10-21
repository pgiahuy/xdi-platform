// SPDX-License-Identifier: Apache-2.0
/* pages/RouterLink.tsx - nối component Link của React Router vào kiểu LinkComponent mà component nhận qua props.
Component trong src/components chỉ biết thẻ a với href; đổi router thì sửa file này và các trang, không sửa
component. */

import type { ComponentProps } from 'react'
import { Link } from 'react-router'

export function RouterLink({ href, ...props }: ComponentProps<'a'> & { href: string }) {
  return <Link to={href} {...props} />
}
