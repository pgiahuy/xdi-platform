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
Hỏi theo cuộc trò chuyện (#164): mỗi câu trả lời thành một lượt xếp dọc (AskConversation.tsx), câu sau gửi kèm
conversation_id của cuộc để mô hình đọc nó như câu tiếp nối; ô hỏi nằm dưới các lượt. Đổi giả định hay trả lời câu hỏi
lại ở lượt cuối thì thay lượt đó, ở lượt cũ hơn thì thêm lượt mới. Nút "Cuộc trò chuyện mới" và đổi dataset bắt đầu cuộc
mới. Pane bằng chứng hiện của lượt đang chọn, mặc định lượt mới nhất.
Ở chế độ dev có công tắc dữ liệu mẫu (api/askSamples.ts); bản build production không có công tắc lẫn file mẫu. */

import { useState } from 'react'
import { Button } from '@/components/ui/button'
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
import { AskConversation } from './AskConversation'
import type { Answer } from './AskOutcome'
import { useEvidenceView } from './evidenceView'
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
  const [turns, setTurns] = useState<Answer[]>([])
  const [selected, setSelected] = useState(0)
  // Lượt đang được hỏi lại; null khi đang hỏi câu mới (hay không hỏi gì).
  const [pendingTurn, setPendingTurn] = useState<number | null>(null)
  const [evidenceOpen, setEvidenceOpen] = useStoredFlag(evidenceOpenKey, true)
  const answer = turns[selected] as Answer | undefined
  const lastTurn = turns.at(-1)
  const evidenceView = useEvidenceView(answer ? evidenceKey(answer) : '')

  // Chưa chọn thì hỏi dataset mới nhất (backend trả mới tạo trước).
  const datasetId = pickedDataset ?? datasets.data?.[0]?.id ?? ''
  const dataset = datasets.data?.find((d) => d.id === datasetId)
  const isAdmin = me.data?.workspaces.find((m) => m.slug === ws)?.role === 'admin'
  const fromSamples = import.meta.env.DEV && sampleMode

  // replace: thay lượt cuối thay vì thêm lượt mới. Lượt nhận được luôn là lượt đang chọn, để pane hiện bằng chứng của
  // câu vừa hỏi.
  const send = (variables: AskVariables, replace: boolean) => {
    setPendingTurn(replace ? turns.length - 1 : null)
    ask.mutate(variables, {
      onSuccess: (response) => {
        const turn = { variables, response }
        const next = replace ? [...turns.slice(0, -1), turn] : [...turns, turn]
        setTurns(next)
        setSelected(next.length - 1)
        // Ô hỏi xóa khi đã có câu trả lời, không xóa khi gửi: lỗi mạng thì người hỏi còn câu để gửi lại.
        if (!replace) setQuestion('')
      },
      onSettled: () => setPendingTurn(null),
    })
  }

  // Response mẫu có id cuộc giả; tắt dữ liệu mẫu rồi hỏi tiếp thì backend phải mở cuộc mới, không nhận id đó.
  const conversationId =
    lastTurn && !lastTurn.variables.fromSamples ? lastTurn.response.conversation_id : undefined

  const askQuestion = (text: string) => {
    const trimmed = text.trim()
    if (!trimmed) return
    // Hỏi lại đúng câu của lượt cuối thì giữ các giả định đã đổi và thay lượt đó; sửa chữ là câu mới.
    const same =
      lastTurn?.variables.datasetId === datasetId &&
      lastTurn.variables.question === trimmed &&
      lastTurn.variables.fromSamples === fromSamples
    const choices = same ? lastTurn.response.choices : []
    send({ datasetId, question: trimmed, choices, conversationId, fromSamples }, same)
  }

  // answered: id các câu hỏi lại vừa trả lời, để nhận ra backend hỏi lại đúng chỗ đó (isRepeatedClarification).
  // fromSamples lấy theo công tắc hiện tại, không theo lần hỏi trước: tắt dữ liệu mẫu rồi bấm chip thì hỏi backend.
  const askAgainWith = (index: number, picks: ChosenOption[], answered: string[] = []) => {
    const turn = turns[index]
    if (!turn) return
    const { datasetId: id, question: text } = turn.variables
    const choices = buildChoicesWith(turn.response.choices, picks)
    send({ datasetId: id, question: text, choices, conversationId, fromSamples, answered }, index === turns.length - 1)
  }

  const startConversation = () => {
    setTurns([])
    setSelected(0)
    ask.reset()
  }

  const changeDataset = (id: string) => {
    setPickedDataset(id)
    startConversation()
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
        answer && (
          // key theo câu trả lời: câu trả lời mới thì pane dựng lại từ đầu; evidenceView cũng bỏ chỗ đã bấm của câu trước.
          <AskEvidence
            key={evidenceKey(answer)}
            ws={ws}
            answer={answer}
            pending={ask.isPending && pendingTurn === selected}
            view={evidenceView}
          />
        )
      }
    >
      {import.meta.env.DEV && <SampleModeSwitch checked={sampleMode} onChange={setSampleMode} />}
      {turns.length > 0 && (
        <>
          <div className="flex items-center justify-between gap-3">
            <p className="text-xs font-semibold tracking-wider text-label uppercase">Cuộc trò chuyện</p>
            <Button variant="outline" size="sm" onClick={startConversation} disabled={ask.isPending}>
              Cuộc trò chuyện mới
            </Button>
          </div>
          <AskConversation
            turns={turns}
            selected={selected}
            onSelect={setSelected}
            pendingTurn={ask.isPending ? pendingTurn : null}
            onCellDrill={(index, target) => {
              setSelected(index)
              evidenceView.drill(target, evidenceKey(turns[index]))
              // Bấm ô thì mở lại pane nếu đang thu gọn: dòng nguồn hiện ở đó.
              setEvidenceOpen(true)
            }}
            onAskAgain={askAgainWith}
          />
        </>
      )}
      {canAsk ? (
        <>
          <AskSection title={turns.length > 0 ? 'Hỏi tiếp' : 'Câu hỏi'}>
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
          {suggestions.length > 0 && turns.length === 0 && (
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
        {announce(ask.isPending, ask.error ? undefined : lastTurn)}
      </p>
      {ask.error && <FormMessage kind="error">{ask.error.message}</FormMessage>}
      {canAsk && turns.length === 0 && !ask.error && (
        <AskIdle pending={ask.isPending} hasSuggestions={suggestions.length > 0} />
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
