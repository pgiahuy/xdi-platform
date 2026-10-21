// SPDX-License-Identifier: Apache-2.0
/* pages/RouterLink.tsx - nối component Link của React Router vào kiểu LinkComponent mà component nhận qua props.
Component chỉ biết thẻ a với href; đổi router thì chỉ sửa file này. */

import type { ComponentProps } from 'react'
import { Link } from 'react-router'

export function RouterLink({ href, ...props }: ComponentProps<'a'> & { href: string }) {
  return <Link to={href} {...props} />
}
