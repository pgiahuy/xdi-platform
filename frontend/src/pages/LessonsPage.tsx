// SPDX-License-Identifier: Apache-2.0
/* pages/LessonsPage.tsx - màn Bài học của workspace (/w/:ws/lessons, #228), chỉ cho admin: hàng số tóm tắt, rồi hai tab,
danh sách bài học (bật là duyệt, chỉ bài đang dùng vào prompt) và nhật ký lỗi mới nhất; hộp viết bài học tay hay từ
một dòng nhật ký. Member mở đường dẫn này thì thấy lời báo; quyền thật do backend kiểm (403). */

import { useState } from 'react'
import { useParams } from 'react-router'
import { Plus } from 'lucide-react'
import { useMe } from '@/api/auth'
import { useDatasets } from '@/api/datasets'
import {
  useCreateLesson,
  useDeleteLesson,
  useErrorLog,
  useLessons,
  useSetLessonEnabled,
  type ErrorEntry,
  type ErrorStep,
  type Lesson,
} from '@/api/lessons'
import { EmptyState } from '@/components/EmptyState'
import { FormMessage } from '@/components/forms/FormMessage'
import { ErrorLogList } from '@/components/lessons/ErrorLogList'
import { LessonDialog } from '@/components/lessons/LessonDialog'
import { LessonList } from '@/components/lessons/LessonList'
import { LessonStats } from '@/components/lessons/LessonStats'
import type { ErrorItem, ErrorTone, LessonDraft, LessonItem, LessonSource } from '@/components/lessons/types'
import { Button } from '@/components/ui/button'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'

const steps: Record<ErrorStep, { label: string; tone: ErrorTone }> = {
  guard: { label: 'Guard từ chối SQL', tone: 'blocked' },
  run: { label: 'SQL lỗi khi chạy', tone: 'failed' },
  fanout: { label: 'Chặn cộng lặp khi nối bảng', tone: 'blocked' },
  why_form: { label: 'Form câu hỏi nguyên nhân sai', tone: 'blocked' },
  narrate: { label: 'Bộ kiểm chặn lời mô hình viết', tone: 'blocked' },
  feedback: { label: 'Người hỏi đánh giá Chưa đạt', tone: 'feedback' },
}

function toErrorItem(entry: ErrorEntry, withLesson: Set<string>): ErrorItem {
  return {
    id: entry.id,
    datasetId: entry.dataset_id,
    datasetName: entry.dataset_name,
    question: entry.question,
    step: entry.step,
    stepLabel: steps[entry.step].label,
    tone: steps[entry.step].tone,
    error: entry.error,
    modelOutput: entry.model_output,
    createdAt: entry.created_at,
    hasLesson: withLesson.has(entry.id),
  }
}

function toLessonItem(lesson: Lesson, questions: Map<string, string>): LessonItem {
  return {
    id: lesson.id,
    text: lesson.text,
    datasetName: lesson.dataset_id ? (lesson.dataset_name ?? 'Dataset đã xóa') : null,
    enabled: lesson.enabled,
    fromError: lesson.error_id !== null,
    fromQuestion: lesson.error_id ? (questions.get(lesson.error_id) ?? null) : null,
    approvedBy: lesson.approved_by,
    approvedAt: lesson.approved_at,
  }
}

type Tab = 'lessons' | 'errors'

export function LessonsPage() {
  const { ws = '' } = useParams()
  const me = useMe()
  const isAdmin = me.data?.workspaces.find((m) => m.slug === ws)?.role === 'admin'
  if (me.data && !isAdmin) {
    return <EmptyState title="Chỉ admin xem được màn này" description="Admin của workspace viết và duyệt bài học từ nhật ký lỗi." />
  }
  return <AdminLessons key={ws} ws={ws} enabled={isAdmin} />
}

function AdminLessons({ ws, enabled }: { ws: string; enabled: boolean }) {
  const errors = useErrorLog(ws, enabled)
  const lessons = useLessons(ws, enabled)
  const datasets = useDatasets(ws)
  const create = useCreateLesson(ws)
  const toggle = useSetLessonEnabled(ws)
  const remove = useDeleteLesson(ws)
  // undefined: chưa bấm tab nào; mở tab bài học, trừ khi chưa có bài nào mà nhật ký có lỗi để viết từ đó.
  const [chosenTab, setChosenTab] = useState<Tab>()
  const [editor, setEditor] = useState<{ draft: LessonDraft; source: LessonSource | null }>()

  const lessonList = lessons.data ?? []
  const errorList = errors.data ?? []
  const withLesson = new Set(lessonList.flatMap((l) => (l.error_id ? [l.error_id] : [])))
  const questions = new Map(errorList.map((e) => [e.id, e.question]))
  const errorItems = errorList.map((e) => toErrorItem(e, withLesson))
  const tab = chosenTab ?? (lessonList.length === 0 && errorList.length > 0 ? 'errors' : 'lessons')

  const open = (draft: LessonDraft, source: LessonSource | null) => {
    create.reset()
    setEditor({ draft, source })
  }
  const submit = (draft: LessonDraft) =>
    create.mutate(
      { text: draft.text, dataset_id: draft.datasetId || null, error_id: draft.errorId },
      {
        // Bài mới chờ duyệt ở tab bài học: chuyển sang đó để admin thấy và bấm Duyệt và bật.
        onSuccess: () => {
          setEditor(undefined)
          setChosenTab('lessons')
        },
      },
    )
  const pendingId = toggle.isPending ? (toggle.variables?.id ?? null) : remove.isPending ? (remove.variables ?? null) : null

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex flex-col gap-1">
          <h2 className="text-lg font-semibold tracking-tight">Bài học</h2>
          <p className="text-sm text-muted-foreground">Câu nhắc mô hình tránh lại lỗi đã gặp.</p>
        </div>
        <Button onClick={() => open({ text: '', datasetId: '', errorId: null }, null)}>
          <Plus aria-hidden />
          Viết bài học
        </Button>
      </div>

      <LessonStats
        enabled={lessonList.filter((l) => l.enabled).length}
        pending={lessonList.filter((l) => !l.enabled && l.approved_at === null).length}
        errors={errorList.length}
        errorsWithoutLesson={errorItems.filter((e) => !e.hasLesson).length}
      />

      <Tabs value={tab} onValueChange={(value) => setChosenTab(value as Tab)} className="min-w-0 gap-4">
        {/* Tab gạch chân xanh ngọc như khung Minh chứng: xanh ngọc là màu của trạng thái đang chọn. */}
        <TabsList variant="line" className="h-10 w-full justify-start gap-6 rounded-none border-b p-0">
          <TabsTrigger
            value="lessons"
            className="flex-none px-0 text-sm text-muted-foreground after:bg-label data-active:font-semibold data-active:text-label"
          >
            Bài học ({lessonList.length})
          </TabsTrigger>
          <TabsTrigger
            value="errors"
            className="flex-none px-0 text-sm text-muted-foreground after:bg-label data-active:font-semibold data-active:text-label"
          >
            Nhật ký lỗi ({errorList.length})
          </TabsTrigger>
        </TabsList>

        <TabsContent value="lessons" className="flex flex-col gap-3">
          {lessons.error && <FormMessage kind="error">{lessons.error.message}</FormMessage>}
          {(toggle.error ?? remove.error) && (
            <FormMessage kind="error">{(toggle.error ?? remove.error)?.message ?? ''}</FormMessage>
          )}
          {lessons.data?.length === 0 && (
            <EmptyState title="Chưa có bài học nào" description="Bấm Viết bài học ở đầu màn hay ở một dòng nhật ký lỗi." />
          )}
          {lessonList.length > 0 && (
            <LessonList
              items={lessonList.map((l) => toLessonItem(l, questions))}
              pendingId={pendingId}
              onToggle={(id, on) => toggle.mutate({ id, enabled: on })}
              onDelete={(id) => remove.mutate(id)}
            />
          )}
        </TabsContent>

        <TabsContent value="errors" className="flex flex-col gap-3">
          {errors.error && <FormMessage kind="error">{errors.error.message}</FormMessage>}
          {errors.data === undefined && !errors.error && <p className="text-sm text-muted-foreground">Đang tải...</p>}
          {errors.data?.length === 0 && <EmptyState title="Chưa có lỗi nào được ghi" />}
          {errorItems.length > 0 && (
            <ErrorLogList
              items={errorItems}
              onWriteLesson={(item) =>
                open(
                  { text: '', datasetId: item.datasetId ?? '', errorId: item.id },
                  { question: item.question, stepLabel: item.stepLabel, error: item.error },
                )
              }
            />
          )}
        </TabsContent>
      </Tabs>

      <LessonDialog
        initial={editor?.draft}
        source={editor?.source ?? null}
        datasets={datasets.data ?? []}
        pending={create.isPending}
        error={create.error ? create.error.message : null}
        onSubmit={submit}
        onClose={() => setEditor(undefined)}
      />
    </div>
  )
}
