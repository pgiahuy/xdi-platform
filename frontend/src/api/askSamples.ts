// SPDX-License-Identifier: Apache-2.0
/* api/askSamples.ts - chỉ dùng ở chế độ dev: trả lời bằng năm response mẫu trong examples/ask.json (sinh từ
backend/src/service/ask/examples.rs bằng make openapi), để dựng và kiểm trang hỏi khi route ask còn trả 501 (#33).
Chỉ được gọi sau điều kiện import.meta.env.DEV, nên bản build production không chứa file này và ask.json.

Số trong mẫu là số minh họa và không đổi theo lựa chọn; chỉ phần lựa chọn được áp dụng như backend sẽ làm (giả định
nhận option đã gửi, choices trả lại đủ mọi lựa chọn), để kiểm được luồng đổi giả định và trả lời câu hỏi lại. */

import type { AskResponse, ChosenOption } from './ask'
import { ApiError } from './client'
import examples from './examples/ask.json'

type SampleName = keyof typeof examples

/** Câu hỏi của từng mẫu, theo chú thích trong ask/examples.rs. Câu về khu vực cho mẫu needs_choice khi chưa chọn
 * cột, mẫu simple khi đã chọn. */
export const sampleQuestions: { question: string; sample: SampleName }[] = [
  { question: 'Tại sao doanh thu quý 3 năm 2017 giảm?', sample: 'why' },
  { question: 'Doanh thu theo khu vực', sample: 'simple' },
  { question: 'Xóa các đơn hàng bị hủy', sample: 'rejected' },
  { question: 'Doanh thu theo ngày giao hàng', sample: 'failed' },
]

// Lựa chọn mà mẫu needs_choice hỏi lại; có nó thì câu về khu vực đã được trả lời.
const regionChoice = 'region_column'

// Chờ một chút như khi gọi backend thật, để thấy được trạng thái đang hỏi.
const delayMs = 400

export async function answerFromSamples(question: string, choices: ChosenOption[]): Promise<AskResponse> {
  await new Promise((resolve) => setTimeout(resolve, delayMs))
  const entry = sampleQuestions.find((sample) => sample.question === question.trim())
  if (!entry) {
    throw new ApiError(0, 'sample', 'Dữ liệu mẫu chỉ trả lời các câu gợi ý bên dưới ô hỏi.')
  }
  const answered = choices.some((choice) => choice.choice_id === regionChoice)
  const name: SampleName = entry.sample === 'simple' && !answered ? 'needs_choice' : entry.sample
  // TypeScript đọc chuỗi trong file JSON là string, không phải enum của hợp đồng; test ask_examples_json_is_up_to_date
  // ở backend bảo đảm file khớp kiểu AskResponse. structuredClone để không sửa mẫu dùng chung.
  const response = structuredClone(examples[name]) as unknown as AskResponse
  return applyChoices(response, choices)
}

function applyChoices(response: AskResponse, choices: ChosenOption[]): AskResponse {
  const assumptions = response.understanding?.assumptions ?? []
  for (const assumption of assumptions) {
    const pick = choices.find((choice) => choice.choice_id === assumption.id)
    if (pick && assumption.options.some((option) => option.id === pick.option_id)) {
      assumption.selected = pick.option_id
    }
  }
  if (assumptions.length > 0) {
    response.choices = assumptions.map((assumption) => ({
      choice_id: assumption.id,
      option_id: assumption.selected ?? '',
    }))
  }
  return response
}
