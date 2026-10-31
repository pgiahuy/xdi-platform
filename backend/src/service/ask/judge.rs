// SPDX-License-Identifier: Apache-2.0
/* service/ask/judge.rs - cổng 2 (#201): gọi mô hình chấm để so lời với các khẳng định đã qua cổng 1, cho lời diễn giải
câu "tại sao" (ask/narrate.rs, prompt dựng ở explain/judge.rs) và lời phần giải thích câu thường (insight/narrate.rs,
prompt dựng ở đó với cùng bộ luật). Schema và cách đọc câu trả lời ở explain/judge.rs. Test qua ask/narrate_tests.rs và
insight/tests.rs.

Lần gọi này độc lập với lần viết lời: prompt mới, không kèm lịch sử, luật hay dữ liệu của lần viết, nên mô hình chấm
không biết lời được dặn viết thế nào, chỉ thấy lời và các khẳng định. Mô hình chấm hiện dùng cùng nhà cung cấp với lần
viết lời (LLM_PROVIDER); dùng nhà cung cấp khác là việc sau. */

use std::time::Duration;

use crate::config::Config;
use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::service::ask::repair::build_user_text;
use crate::service::ask::run::build_duration_text;
use crate::service::ask::why::build_narrate_timeout;
use crate::service::explain::judge::{build_judge_schema, parse_judge_reply, Verdict};

/// Thời gian chờ mô hình chấm: như lần viết lời, vì cùng nhà cung cấp.
pub fn build_judge_timeout(config: &Config) -> Duration {
    build_narrate_timeout(config)
}

/// prompt do bên gọi dựng (build_judge_prompt hay build_insight_judge_prompt). Bộ đo mô hình chấm (service/eval_judge.rs)
/// gọi cùng hàm này. Err khi không chấm được (mô hình lỗi, quá giờ, câu trả lời không đọc được), kèm lý do cho bước
/// của bên gọi. Bên gọi không coi Err là khớp.
pub async fn judge(
    llm: &dyn LlmProvider,
    question: &str,
    prompt: &str,
    timeout: Duration,
) -> Result<Verdict, String> {
    let request = LlmRequest {
        task: LlmTask::Judge,
        question,
        prompt,
        json_schema: Some(build_judge_schema()),
    };
    match tokio::time::timeout(timeout, llm.complete(&request)).await {
        Ok(Ok(reply)) => parse_judge_reply(&reply),
        Ok(Err(error)) => Err(format!(
            "Mô hình chấm không trả lời được: {}",
            build_user_text(&error, "judge")
        )),
        Err(_) => Err(format!(
            "Mô hình chấm không trả lời trong {}",
            build_duration_text(timeout)
        )),
    }
}
