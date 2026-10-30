// SPDX-License-Identifier: Apache-2.0
/* pages/AskPage.tsx - trang đầu của một workspace: hỏi dữ liệu. Chọn dataset, gõ câu hỏi, xem thẻ "Đã hiểu là", đổi
giả định hoặc trả lời câu hỏi lại; phần hiện response ở AskOutcome.tsx, pane bằng chứng ở AskEvidence.tsx.
Quy tắc gửi lựa chọn theo hợp đồng (ARCHITECTURE mục 8): đổi giả định hay trả lời câu hỏi lại thì gửi lại đúng câu
hỏi đó kèm choices của response gần nhất (buildChoicesWith); gõ câu khác là câu mới, choices rỗng. Response gần nhất
giữ trong state kèm request đã gửi, vì hỏi lại cần cả hai và useMutation xóa data khi bắt đầu lần gọi mới.
Câu gợi ý lấy từ dataset đang chọn (suggested_questions, lớp hồ sơ ngữ nghĩa viết sau khi nạp); danh sách dataset tự
tải lại khi còn dataset đang lập hồ sơ (useDatasets, watchProfiles), nên câu gợi ý hiện ra mà không phải tải lại trang.
Chưa hỏi gì thì khung trò chuyện là một dòng nhắc bước tiếp theo (AskIdle) cùng câu gợi ý. Pane bằng chứng chỉ hiện
khi đã có câu trả lời; mở hay thu gọn nhớ trong localStorage (storedFlag.ts) cho mọi workspace, vì đó là thói quen của
người dùng, không phải của từng dataset.
Hỏi theo cuộc trò chuyện (#164): mỗi câu trả lời thành một lượt của khung trò chuyện (AskConversation.tsx), câu sau
gửi kèm conversation_id của cuộc để mô hình đọc nó như câu tiếp nối; ô hỏi dính đáy cột chính. Đổi giả định hay trả lời
câu hỏi lại ở lượt cuối thì thay lượt đó, ở lượt cũ hơn thì thêm lượt mới. Nút "Cuộc trò chuyện mới" và đổi dataset bắt
đầu cuộc mới. Pane bằng chứng hiện của lượt đang chọn, mặc định lượt mới nhất.
Danh sách cuộc cũ (api/conversations.ts) ở cột trái màn rộng, trong ngăn kéo màn hẹp. Mở lại một cuộc thì hiện các
lượt đã lưu (chỉ câu hỏi và tóm tắt, backend không lưu bảng kết quả) và hỏi tiếp vào cuộc đó. Đường dẫn mang dataset và
id cuộc (?dataset=...&c=...), nên tải lại trang mở đúng cuộc; cuộc vừa tạo bằng câu đầu tiên cũng ghi id vào đường dẫn.
Chỉ cuộc mở từ đường dẫn hay từ danh sách (openedId) mới tải lượt đã lưu: cuộc đang hỏi trong phiên đã có đủ response.
Ở chế độ dev có công tắc dữ liệu mẫu (api/askSamples.ts); bản build production không có công tắc lẫn file mẫu. */

import { useState } from 'react'
import { History, Plus } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Sheet, SheetContent, SheetDescription, SheetTitle, SheetTrigger } from '@/components/ui/sheet'
import { useParams, useSearchParams } from 'react-router'
import { buildChoicesWith, isRepeatedClarification, useAsk, type AskVariables, type ChosenOption } from '@/api/ask'
import { sampleQuestions } from '@/api/askSamples'
import { useMe } from '@/api/auth'
import { useConversation, useConversations } from '@/api/conversations'
import { isProfiling, useDatasets, type Dataset } from '@/api/datasets'
import { AskIdle } from '@/components/ask/AskIdle'
import { AskLayout } from '@/components/ask/AskLayout'
import { ConversationList } from '@/components/ask/ConversationList'
import { DatasetPicker } from '@/components/ask/DatasetPicker'
import { QuestionForm } from '@/components/ask/QuestionForm'
import { SampleModeSwitch } from '@/components/ask/SampleModeSwitch'
import { SuggestedQuestions } from '@/components/ask/SuggestedQuestions'
import type { ProfileProgress } from '@/components/ask/types'
import { FormMessage } from '@/components/forms/FormMessage'
import { OverviewToggle } from '@/components/overview/OverviewToggle'
import { AskDatasetStatus } from './AskDatasetStatus'
import { AskEvidence } from './AskEvidence'
import { AskOverview, askOverviewId } from './AskOverview'
import { AskConversation } from './AskConversation'
import type { Answer } from './AskOutcome'
import { useEvidenceView } from './evidenceView'
import { useStoredFlag } from './storedFlag'
import { workspaceHref } from './workspaceNav'

const evidenceOpenKey = 'xdi.ask.evidenceOpen'
const historyColumnKey = 'xdi.ask.historyOpen'

export function AskPage() {
  const { ws = '' } = useParams()
  // Đổi workspace vẫn giữ trang này (cùng route); key theo workspace để câu hỏi và câu trả lời cũ không đi theo.
  return <WorkspaceAsk key={ws} ws={ws} />
}

function WorkspaceAsk({ ws }: { ws: string }) {
  const me = useMe()
  const datasets = useDatasets(ws, { watchProfiles: true })
  const ask = useAsk(ws)
  const [params, setParams] = useSearchParams()
  // ?q= điền sẵn ô hỏi: trang Dữ liệu mở trang này với câu bấm từ khung tổng quan (#198).
  const [question, setQuestion] = useState(() => params.get('q') ?? '')
  const [overviewOpen, setOverviewOpen] = useState(false)
  const [pickedDataset, setPickedDataset] = useState(() => params.get('dataset') ?? undefined)
  // Cuộc mở lại từ đường dẫn hay từ danh sách; null khi đang ở cuộc mới (kể cả cuộc vừa tạo trong phiên này).
  const [openedId, setOpenedId] = useState(() => params.get('c'))
  const [historyOpen, setHistoryOpen] = useState(false)
  const [sampleMode, setSampleMode] = useState(false)
  const [turns, setTurns] = useState<Answer[]>([])
  const [selected, setSelected] = useState(0)
  // Lượt đang được hỏi lại; null khi đang hỏi câu mới (hay không hỏi gì).
  const [pendingTurn, setPendingTurn] = useState<number | null>(null)
  const [evidenceOpen, setEvidenceOpen] = useStoredFlag(evidenceOpenKey, true)
  // Cột danh sách cuộc trò chuyện (màn rộng) ẩn hay hiện: thói quen của người dùng như pane minh chứng.
  const [historyColumn, setHistoryColumn] = useStoredFlag(historyColumnKey, true)
  const answer = turns[selected] as Answer | undefined
  const lastTurn = turns.at(-1)
  const evidenceView = useEvidenceView(answer ? evidenceKey(answer) : '')

  // Chưa chọn thì hỏi dataset mới nhất (backend trả mới tạo trước); dataset trong đường dẫn đã bị xóa cũng vậy.
  const picked = datasets.data?.find((d) => d.id === pickedDataset)
  const dataset = picked ?? datasets.data?.[0]
  const datasetId = dataset?.id ?? ''
  const conversations = useConversations(ws, datasetId)
  // Dataset trong đường dẫn không còn thì id cuộc đi kèm cũng không mở được.
  const opened = useConversation(ws, datasetId, picked || !pickedDataset ? openedId : null)
  const savedTurns = opened.data?.turns ?? []
  const isAdmin = me.data?.workspaces.find((m) => m.slug === ws)?.role === 'admin'
  const fromSamples = import.meta.env.DEV && sampleMode

  // replace: thay lượt cuối thay vì thêm lượt mới. Lượt nhận được luôn là lượt đang chọn, để pane hiện minh chứng của
  // câu vừa hỏi.
  const send = (variables: AskVariables, replace: boolean) => {
    setPendingTurn(replace ? turns.length - 1 : null)
    ask.mutate(variables, {
      // Ô hỏi đã xóa lúc gửi (askQuestion); lỗi mạng hay lỗi server thì trả lại câu vào ô để gửi lại, trừ khi người
      // dùng đã gõ câu khác trong lúc chờ.
      onError: () => setQuestion((current) => (current === '' ? variables.question : current)),
      onSuccess: (response) => {
        const turn = { variables, response }
        const next = replace ? [...turns.slice(0, -1), turn] : [...turns, turn]
        setTurns(next)
        setSelected(next.length - 1)
        // Ghi id cuộc vào đường dẫn để tải lại trang mở đúng cuộc; id cuộc của response mẫu là id giả.
        if (!variables.fromSamples) writeUrl(variables.datasetId, response.conversation_id, true)
      },
      onSettled: () => setPendingTurn(null),
    })
  }

  // Response mẫu có id cuộc giả; tắt dữ liệu mẫu rồi hỏi tiếp thì backend phải mở cuộc mới, không nhận id đó.
  // Chưa hỏi gì trong phiên mà đang mở lại một cuộc thì hỏi tiếp vào cuộc đó.
  const conversationId = lastTurn
    ? lastTurn.variables.fromSamples
      ? undefined
      : lastTurn.response.conversation_id
    : (opened.data?.conversation.id ?? undefined)

  const askQuestion = (text: string) => {
    const trimmed = text.trim()
    if (!trimmed) return
    // Hỏi lại đúng câu của lượt cuối thì giữ các giả định đã đổi và thay lượt đó; sửa chữ là câu mới.
    const same =
      lastTurn?.variables.datasetId === datasetId &&
      lastTurn.variables.question === trimmed &&
      lastTurn.variables.fromSamples === fromSamples
    const choices = same ? lastTurn.response.choices : []
    // Xóa ô ngay khi gửi: câu đã hiện thành bong bóng của lượt đang chờ, để lại trong ô dễ tưởng chưa gửi. Hỏi lại một
    // lượt đã lưu thì ô có thể đang chứa câu khác người dùng đang gõ: chỉ xóa khi ô chứa đúng câu vừa gửi.
    setQuestion((current) => (current.trim() === trimmed ? '' : current))
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

  // replace: thay mục lịch sử của trình duyệt thay vì thêm, cho thay đổi trang tự làm (cuộc vừa tạo có id).
  const writeUrl = (dataset: string, conversation: string | null, replace = false) => {
    const next = new URLSearchParams({ dataset })
    if (conversation) next.set('c', conversation)
    setParams(next, { replace })
  }

  const clearTurns = () => {
    setTurns([])
    setSelected(0)
    ask.reset()
    setHistoryOpen(false)
  }

  const startConversation = () => {
    clearTurns()
    setOpenedId(null)
    writeUrl(datasetId, null)
  }

  const openConversation = (id: string) => {
    clearTurns()
    // Bấm lại đúng cuộc đang mở: khóa không đổi nên useConversation không tự tải, mà các lượt vừa hỏi đã bị xóa khỏi
    // màn; tải lại để chúng hiện thành lượt đã lưu.
    if (id === openedId) void opened.refetch()
    setOpenedId(id)
    writeUrl(datasetId, id)
  }

  const changeDataset = (id: string) => {
    setPickedDataset(id)
    clearTurns()
    setOpenedId(null)
    writeUrl(id, null)
  }

  const showEvidence = (index: number) => {
    setSelected(index)
    setEvidenceOpen(true)
    // Màn hẹp: pane nằm dưới khung trò chuyện, cuộn tới đó; màn rộng pane dính bên phải nên không phải cuộn.
    document.getElementById('evidence')?.scrollIntoView({ block: 'nearest' })
  }

  const pickSuggestion = (text: string) => askQuestion(text)

  const hasDatasets = (datasets.data?.length ?? 0) > 0
  const suggestions = fromSamples
    ? sampleQuestions.map((sample) => sample.question)
    : (dataset?.suggested_questions ?? [])
  const canAsk = hasDatasets || fromSamples
  const empty = turns.length === 0 && savedTurns.length === 0
  const pendingQuestion = ask.isPending && pendingTurn === null ? (ask.variables?.question ?? null) : null
  const history = hasDatasets && (
    <ConversationList
      conversations={(conversations.data ?? []).map((c) => ({ id: c.id, title: c.title, createdAt: c.created_at }))}
      // Cuộc vừa tạo trong phiên cũng là cuộc đang mở.
      activeId={openedId ?? (conversationId || null)}
      loading={conversations.isPending && datasetId !== ''}
      error={conversations.error ? 'Không tải được danh sách cuộc trò chuyện.' : null}
      onOpen={openConversation}
      onNew={startConversation}
      disabled={ask.isPending}
    />
  )
  return (
    <AskLayout
      history={history}
      historyOpen={historyColumn}
      onHistoryOpenChange={setHistoryColumn}
      historyHiddenActions={
        <Button variant="outline" size="sm" onClick={startConversation} disabled={ask.isPending || empty}>
          <Plus aria-hidden />
          Cuộc trò chuyện mới
        </Button>
      }
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
      {canAsk && (
        // Dòng dataset dính dưới thanh trên (h-16) khi cuộn cuộc trò chuyện dài: luôn thấy đang hỏi trên dataset nào.
        <div className="sticky top-16 z-10 -mx-4 -mt-4 bg-secondary px-4 pt-4 pb-2 sm:-mx-5 sm:-mt-5 sm:px-5 sm:pt-5">
          <DatasetPicker
            datasets={(datasets.data ?? []).map((d) => ({ id: d.id, name: d.name }))}
            datasetId={datasetId}
            onDatasetChange={changeDataset}
            // Dữ liệu mẫu không qua dataset thật, báo hồ sơ của dataset đang chọn ở đó chỉ gây nhầm.
            profile={fromSamples ? 'ready' : buildProfileProgress(dataset)}
            disabled={ask.isPending}
            action={
              <OverviewToggle
                open={overviewOpen}
                onToggle={() => setOverviewOpen((open) => !open)}
                controls={askOverviewId}
                disabled={fromSamples || datasetId === ''}
              />
            }
          />
        </div>
      )}
      {overviewOpen && !fromSamples && datasetId !== '' && (
        <AskOverview ws={ws} datasetId={datasetId} onPick={setQuestion} onClose={() => setOverviewOpen(false)} />
      )}
      {import.meta.env.DEV && <SampleModeSwitch checked={sampleMode} onChange={setSampleMode} />}
      {hasDatasets && (
        // Dưới lg cột danh sách ẩn: hai nút này thay chỗ của nó.
        <div className="flex flex-wrap items-center gap-2 lg:hidden">
          <Sheet open={historyOpen} onOpenChange={setHistoryOpen}>
            <SheetTrigger asChild>
              <Button variant="outline" size="sm">
                <History aria-hidden />
                Các cuộc trò chuyện
              </Button>
            </SheetTrigger>
            <SheetContent side="left" className="w-72 max-w-[85vw] overflow-y-auto p-4">
              <SheetTitle>Cuộc trò chuyện</SheetTitle>
              <SheetDescription className="sr-only">Mở lại một cuộc trò chuyện cũ trên dataset đang chọn</SheetDescription>
              {history}
            </SheetContent>
          </Sheet>
          <Button variant="outline" size="sm" onClick={startConversation} disabled={ask.isPending || empty}>
            <Plus aria-hidden />
            Cuộc trò chuyện mới
          </Button>
        </div>
      )}
      {opened.isPending && openedId !== null && datasetId !== '' && (
        <p className="text-sm text-muted-foreground">Đang mở cuộc trò chuyện...</p>
      )}
      {opened.error && <FormMessage kind="error">{`Không mở được cuộc trò chuyện: ${opened.error.message}`}</FormMessage>}
      {(!empty || pendingQuestion !== null) && (
        <AskConversation
          savedTurns={savedTurns}
          turns={turns}
          selected={selected}
          onSelect={setSelected}
          pendingTurn={ask.isPending ? pendingTurn : null}
          pendingQuestion={pendingQuestion}
          onCellDrill={(index, target) => {
            setSelected(index)
            evidenceView.drill(target, evidenceKey(turns[index]))
            // Bấm ô thì mở lại pane nếu đang thu gọn: dòng nguồn hiện ở đó.
            setEvidenceOpen(true)
          }}
          onAskAgain={askAgainWith}
          onShowEvidence={showEvidence}
          onAskSaved={(text) => askQuestion(text)}
          pending={ask.isPending}
        />
      )}
      {canAsk && empty && !ask.isPending && !opened.isFetching && (
        <div className="flex flex-col gap-4">
          {!ask.error && <AskIdle pending={false} hasSuggestions={suggestions.length > 0} />}
          {suggestions.length > 0 && (
            <section aria-labelledby="ask-suggestions" className="flex flex-col gap-2">
              <h2 id="ask-suggestions" className="text-xs font-semibold tracking-wider text-label uppercase">
                Câu gợi ý
              </h2>
              <SuggestedQuestions questions={suggestions} onPick={pickSuggestion} disabled={ask.isPending} />
            </section>
          )}
        </div>
      )}
      {/* Báo cho trình đọc màn hình khi đang hỏi và khi câu trả lời về; vùng này luôn có trong trang, vì vùng
      aria-live mới chèn vào thì trình đọc màn hình thường không đọc. Lỗi, rejected, failed đã có role alert. */}
      <p role="status" className="sr-only">
        {announce(ask.isPending, ask.error ? undefined : lastTurn)}
      </p>
      {ask.error && <FormMessage kind="error">{ask.error.message}</FormMessage>}
      {canAsk ? (
        // Ô hỏi dính đáy khung trò chuyện khi cuộn; nền đặc cùng màu khung che phần trả lời trôi bên dưới.
        <div className="sticky bottom-0 z-10 mt-auto bg-secondary pt-1">
          <div className="rounded-xl border bg-card p-3 text-card-foreground shadow-sm">
            <QuestionForm
              question={question}
              onQuestionChange={setQuestion}
              onSubmit={() => askQuestion(question)}
              pending={ask.isPending}
            />
          </div>
        </div>
      ) : (
        <AskDatasetStatus
          error={datasets.error}
          loading={datasets.isPending}
          onRetry={() => datasets.refetch()}
          isAdmin={isAdmin}
          datasetsHref={workspaceHref(ws, 'datasets')}
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
