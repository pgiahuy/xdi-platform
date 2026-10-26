// SPDX-License-Identifier: Apache-2.0
/* pages/AskPage.tsx - trang đầu của một workspace: hỏi dữ liệu. Chọn dataset, gõ câu hỏi, xem thẻ "Đã hiểu là", đổi
giả định hoặc trả lời câu hỏi lại; phần hiện response ở AskOutcome.tsx, pane bằng chứng ở AskEvidence.tsx.
Quy tắc gửi lựa chọn theo hợp đồng (ARCHITECTURE mục 8): đổi giả định hay trả lời câu hỏi lại thì gửi lại đúng câu
hỏi đó kèm choices của response gần nhất (buildChoicesWith); gõ câu khác là câu mới, choices rỗng. Response gần nhất
giữ trong state kèm request đã gửi, vì hỏi lại cần cả hai và useMutation xóa data khi bắt đầu lần gọi mới.
Câu gợi ý lấy từ dataset đang chọn (suggested_questions, lớp hồ sơ ngữ nghĩa viết sau khi nạp); danh sách dataset tự
tải lại khi còn dataset đang lập hồ sơ (useDatasets, watchProfiles), nên câu gợi ý hiện ra mà không phải tải lại trang.
Mỗi khối (câu hỏi, câu gợi ý, kết quả, bằng chứng) nằm trong một thẻ riêng; chưa hỏi gì thì chỗ kết quả là một dòng
nhắc bước tiếp theo (AskIdle). Pane bằng chứng chỉ hiện khi đã có câu trả lời; mở hay thu gọn nhớ trong localStorage
(storedFlag.ts) cho mọi workspace, vì đó là thói quen của người dùng, không phải của từng dataset.
Ở chế độ dev có công tắc dữ liệu mẫu (api/askSamples.ts); bản build production không có công tắc lẫn file mẫu. */

import { useState } from 'react'
import { useParams } from 'react-router'
import { buildChoicesWith, isRepeatedClarification, useAsk, type AskVariables, type ChosenOption } from '@/api/ask'
import { sampleQuestions } from '@/api/askSamples'
import { useMe } from '@/api/auth'
import { isProfiling, useDatasets, type Dataset } from '@/api/datasets'
import { AskIdle } from '@/components/ask/AskIdle'
import { AskLayout } from '@/components/ask/AskLayout'
import { AskSection } from '@/components/ask/AskSection'
import { QuestionForm } from '@/components/ask/QuestionForm'
import { SampleModeSwitch } from '@/components/ask/SampleModeSwitch'
import { SuggestedQuestions } from '@/components/ask/SuggestedQuestions'
import type { ProfileProgress } from '@/components/ask/types'
import { FormMessage } from '@/components/forms/FormMessage'
import { AskDatasetStatus } from './AskDatasetStatus'
import { AskEvidence } from './AskEvidence'
import { AskOutcome, type Answer } from './AskOutcome'
import { useStoredFlag } from './storedFlag'
import { workspaceHref } from './workspaceNav'

const evidenceOpenKey = 'xdi.ask.evidenceOpen'

export function AskPage() {
  const { ws = '' } = useParams()
  // Đổi workspace vẫn giữ trang này (cùng route); key theo workspace để câu hỏi và câu trả lời cũ không đi theo.
  return <WorkspaceAsk key={ws} ws={ws} />
}

function WorkspaceAsk({ ws }: { ws: string }) {
  const me = useMe()
  const datasets = useDatasets(ws, { watchProfiles: true })
  const ask = useAsk(ws)
  const [question, setQuestion] = useState('')
  const [pickedDataset, setPickedDataset] = useState<string>()
  const [sampleMode, setSampleMode] = useState(false)
  const [answer, setAnswer] = useState<Answer>()
  const [evidenceOpen, setEvidenceOpen] = useStoredFlag(evidenceOpenKey, true)

  // Chưa chọn thì hỏi dataset mới nhất (backend trả mới tạo trước).
  const datasetId = pickedDataset ?? datasets.data?.[0]?.id ?? ''
  const dataset = datasets.data?.find((d) => d.id === datasetId)
  const isAdmin = me.data?.workspaces.find((m) => m.slug === ws)?.role === 'admin'
  const fromSamples = import.meta.env.DEV && sampleMode

  const send = (variables: AskVariables) => {
    ask.mutate(variables, { onSuccess: (response) => setAnswer({ variables, response }) })
  }

  const askQuestion = (text: string) => {
    const trimmed = text.trim()
    if (!trimmed) return
    // Hỏi lại đúng câu vừa hỏi trên cùng dataset thì giữ các giả định đã đổi; sửa chữ là câu mới.
    const same =
      answer?.variables.datasetId === datasetId &&
      answer.variables.question === trimmed &&
      answer.variables.fromSamples === fromSamples
    const choices = same ? answer.response.choices : []
    send({ datasetId, question: trimmed, choices, fromSamples })
  }

  // answered: id các câu hỏi lại vừa trả lời, để nhận ra backend hỏi lại đúng chỗ đó (isRepeatedClarification).
  // fromSamples lấy theo công tắc hiện tại, không theo lần hỏi trước: tắt dữ liệu mẫu rồi bấm chip thì hỏi backend.
  const askAgainWith = (picks: ChosenOption[], answered: string[] = []) => {
    if (!answer) return
    const { datasetId: id, question: text } = answer.variables
    const choices = buildChoicesWith(answer.response.choices, picks)
    send({ datasetId: id, question: text, choices, fromSamples, answered })
  }

  const changeDataset = (id: string) => {
    setPickedDataset(id)
    setAnswer(undefined)
    ask.reset()
  }

  const pickSuggestion = (text: string) => {
    setQuestion(text)
    askQuestion(text)
  }

  const hasDatasets = (datasets.data?.length ?? 0) > 0
  const suggestions = fromSamples
    ? sampleQuestions.map((sample) => sample.question)
    : (dataset?.suggested_questions ?? [])
  const canAsk = hasDatasets || fromSamples
  return (
    <AskLayout
      evidenceOpen={evidenceOpen}
      onEvidenceOpenChange={setEvidenceOpen}
      evidence={
        answer &&
        !ask.error && (
          // key theo câu trả lời: câu trả lời mới thì pane về tab mặc định, bỏ dòng phân rã đã bấm của câu trước.
          <AskEvidence key={evidenceKey(answer)} ws={ws} answer={answer} pending={ask.isPending} />
        )
      }
    >
      {import.meta.env.DEV && <SampleModeSwitch checked={sampleMode} onChange={setSampleMode} />}
      {canAsk ? (
        <>
          <AskSection title="Câu hỏi">
            <QuestionForm
              datasets={(datasets.data ?? []).map((d) => ({ id: d.id, name: d.name }))}
              datasetId={datasetId}
              onDatasetChange={changeDataset}
              // Dữ liệu mẫu không qua dataset thật, báo hồ sơ của dataset đang chọn ở đó chỉ gây nhầm.
              profile={fromSamples ? 'ready' : buildProfileProgress(dataset)}
              question={question}
              onQuestionChange={setQuestion}
              onSubmit={() => askQuestion(question)}
              pending={ask.isPending}
            />
          </AskSection>
          {suggestions.length > 0 && (
            <AskSection title="Câu gợi ý">
              <SuggestedQuestions questions={suggestions} onPick={pickSuggestion} disabled={ask.isPending} />
            </AskSection>
          )}
        </>
      ) : (
        <AskDatasetStatus
          error={datasets.error}
          loading={datasets.isPending}
          onRetry={() => datasets.refetch()}
          isAdmin={isAdmin}
          datasetsHref={workspaceHref(ws, 'datasets')}
        />
      )}
      {/* Báo cho trình đọc màn hình khi đang hỏi và khi câu trả lời về; vùng này luôn có trong trang, vì vùng
      aria-live mới chèn vào thì trình đọc màn hình thường không đọc. Lỗi, rejected, failed đã có role alert. */}
      <p role="status" className="sr-only">
        {announce(ask.isPending, ask.error ? undefined : answer)}
      </p>
      {ask.error && <FormMessage kind="error">{ask.error.message}</FormMessage>}
      {canAsk && !answer && !ask.error && <AskIdle pending={ask.isPending} hasSuggestions={suggestions.length > 0} />}
      {/* Lỗi (501, 502, 503, mạng) hiện thay cho câu trả lời cũ, để không ai tưởng câu trả lời cũ là kết quả mới. Câu
      trả lời cũ vẫn giữ trong state: hỏi lại đúng câu đó sau khi lỗi thì vẫn giữ các giả định đã đổi. */}
      {answer && !ask.error && (
        <AskOutcome
          answer={answer}
          pending={ask.isPending}
          onChangeAssumption={(choiceId, optionId) => askAgainWith([{ choice_id: choiceId, option_id: optionId }])}
          onAnswerClarifications={(picks) =>
            askAgainWith(
              picks.map((pick) => ({
                choice_id: pick.choiceId,
                option_id: pick.optionId,
              })),
              picks.map((pick) => pick.choiceId),
            )
          }
        />
      )}
    </AskLayout>
  )
}

// query_id là null với needs_choice, và response mẫu giữ cùng query_id khi đổi giả định: thêm request đã gửi để mỗi lần
// hỏi là một khóa riêng.
function evidenceKey({ variables, response }: Answer): string {
  return `${response.query_id ?? ''}:${JSON.stringify(variables)}`
}

function buildProfileProgress(dataset: Dataset | undefined): ProfileProgress {
  if (dataset && isProfiling(dataset)) return 'profiling'
  if (dataset?.current_profile_status === 'failed') return 'failed'
  if (dataset?.semantic_unavailable) return 'no_suggestions'
  return 'ready'
}

function announce(pending: boolean, answer: Answer | undefined): string {
  if (pending) return 'Đang hỏi...'
  if (answer?.response.status === 'answered') return 'Đã có câu trả lời.'
  if (answer?.response.status === 'needs_choice') {
    // Bị hỏi lại đúng chỗ vừa trả lời thì trang chỉ hiện lời báo lỗi (role="alert"), không có form chọn: không đọc thêm
    // câu bảo chọn, tránh hai câu mâu thuẫn.
    if (isRepeatedClarification(answer.variables.answered ?? [], answer.response)) return ''
    return 'Câu hỏi hiểu được theo nhiều cách, cần chọn cách hiểu.'
  }
  return ''
}
