// SPDX-License-Identifier: Apache-2.0
/* api/schema.ts - FILE SINH TỰ ĐỘNG, không sửa tay. Kiểu TypeScript của mọi endpoint và schema trong openapi.json ở
gốc repo, sinh bằng openapi-typescript (frontend/scripts/api-types.mjs). Đổi API: sửa backend, chạy make openapi, commit
cả openapi.json và file này. Các hook trong src/api lấy kiểu từ đây, không tự khai lại. */

export interface paths {
    "/admin/users": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Every account (system administrator only) */
        get: operations["listUsers"];
        put?: never;
        /**
         * Create an account (system administrator only)
         * @description The new account belongs to no workspace until a workspace admin adds it.
         */
        post: operations["createUser"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/users/{id}/activate": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** Reactivate an account (system administrator only) */
        post: operations["activateUser"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/users/{id}/deactivate": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** Deactivate an account and sign it out everywhere (system administrator only) */
        post: operations["deactivateUser"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/users/{id}/reset-password": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** Set a temporary password and sign the account out everywhere (system administrator only) */
        post: operations["resetUserPassword"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/workspaces": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Every workspace with its members (system administrator only)
         * @description Names and members only, none of the workspace data.
         */
        get: operations["listAllWorkspaces"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/workspaces/{id}/admins": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Make an account admin of any workspace (system administrator only)
         * @description For a workspace that lost its last admin. Each assignment is written to the server log.
         */
        post: operations["assignWorkspaceAdmin"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/auth/demo-login": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** Sign in to the public demo with a role (not implemented yet) */
        post: operations["demoLogin"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/auth/login": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Sign in with email and password
         * @description Sets the session cookie. A wrong email and a wrong password give the same 401 message.
         */
        post: operations["login"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/auth/logout": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Sign out: delete the session and clear the cookie
         * @description Succeeds even when not signed in.
         */
        post: operations["logout"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/auth/me": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** The signed-in user and their workspaces */
        get: operations["me"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/auth/password": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Change your own password
         * @description The current session stays signed in; every other session of the account is signed out.
         */
        post: operations["changePassword"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/auth/setup": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Whether the first-run administrator setup is still needed */
        get: operations["setupStatus"];
        put?: never;
        /**
         * Create the first administrator account and workspace, then sign in
         * @description Only works while no account exists and with the setup code from the backend log. Sets the session cookie.
         */
        post: operations["setup"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/health": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Liveness check for compose and CI */
        get: operations["health"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/version": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Server name, version and language model provider */
        get: operations["version"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Workspaces of the signed-in user, with their role in each */
        get: operations["listWorkspaces"];
        put?: never;
        /** Create a workspace; the creator becomes its admin */
        post: operations["createWorkspace"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        /** Delete the workspace with all its datasets and files (admin only) */
        delete: operations["deleteWorkspace"];
        options?: never;
        head?: never;
        /** Rename the workspace (admin only); the slug does not change */
        patch: operations["renameWorkspace"];
        trace?: never;
    };
    "/workspaces/{ws}/datasets": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Datasets of the workspace */
        get: operations["listDatasets"];
        put?: never;
        /**
         * Upload files as a new dataset or a new version of one (admin only)
         * @description Each upload is a version. Uploading exactly the files of an existing version creates nothing and returns 200 with `created` false.
         */
        post: operations["uploadDataset"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/datasets/sheets": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Sheets of an Excel file, to choose which to upload (admin only)
         * @description The file is read and discarded.
         */
        post: operations["listExcelSheets"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/datasets/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** One dataset of the workspace */
        get: operations["getDataset"];
        put?: never;
        post?: never;
        /** Delete a dataset with all its versions and files (admin only) */
        delete: operations["deleteDataset"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/datasets/{id}/ask": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Ask a question about a dataset
         * @description One JSON response, no streaming. A question that cannot be answered (`rejected`, `needs_choice`, `failed`) is still 200, with `status` and `message` in the body and the evidence gathered so far. Error codes are for invalid requests, access, and the language model service.
         */
        post: operations["ask"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/datasets/{id}/conversations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Conversations of the caller on a dataset
         * @description Newest first. Conversations with no recorded question are left out.
         */
        get: operations["listConversations"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/datasets/{id}/conversations/{conversation_id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * One conversation of the caller with its turns
         * @description Turns oldest first, each with its question, status, SQL and the short text of what was understood. Results are not stored: ask a turn again to see its numbers.
         */
        get: operations["getConversation"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/datasets/{id}/drill": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Source rows behind a number of an answer
         * @description Reads the stored question, keeps its table and filters, adds the keys of the clicked cell, and runs that on the dataset version the question used, through the guard with the row filters of the member calling now. One page of rows per call, in the order the analytics engine reads them.
         */
        post: operations["drill"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/members": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Members of the workspace */
        get: operations["listMembers"];
        put?: never;
        /** Add an existing account to the workspace by email (admin only) */
        post: operations["addMember"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/members/{user_id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        /**
         * Remove a member (admin), or leave the workspace (yourself)
         * @description The workspace always keeps at least one admin.
         */
        delete: operations["removeMember"];
        options?: never;
        head?: never;
        /**
         * Change a member's role (admin only)
         * @description The workspace always keeps at least one admin.
         */
        patch: operations["setMemberRole"];
        trace?: never;
    };
    "/workspaces/{ws}/sources": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Database sources of the workspace, without passwords (admin only) */
        get: operations["listSources"];
        put?: never;
        /**
         * Add a Postgres source (admin only)
         * @description The connection is tested before it is saved. The host must be listed in SOURCE_ALLOWED_HOSTS. Connection details are stored encrypted with SOURCE_SECRET_KEY.
         */
        post: operations["createSource"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/sources/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        /** Delete a source that has no datasets left (admin only) */
        delete: operations["deleteSource"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/sources/{id}/relations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Foreign keys around a table: many-to-one joins it can take, and incoming ones it cannot (admin only)
         * @description Read from pg_catalog. Many-to-one relations follow foreign keys leaving the table, and one more hop from each joinable table; a relation is joinable when the target columns are the primary key or a UNIQUE constraint and the account can read the target. Foreign keys pointing into the table are listed in `one_to_many` and never joined, since they would repeat its rows.
         */
        post: operations["listSourceRelations"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/sources/{id}/snapshot": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Snapshot a table as a new dataset, or as a new version of one (admin only)
         * @description The table is copied once into a Parquet version, like an uploaded file. With `relations`, the chosen many-to-one tables are LEFT JOINed in one COPY statement; their columns are named `<table>.<column>`, and a join that changes the row count is refused. A snapshot again (`dataset_id`) reuses the joins of the current version. A snapshot with the same content as an existing version creates nothing and returns 200 with `created` false.
         */
        post: operations["snapshotSource"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/sources/{id}/tables": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Tables and views the source account can read (admin only) */
        get: operations["listSourceTables"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/sources/{id}/test": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** Connect to the source and count readable tables (admin only) */
        post: operations["testSource"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
}
export type webhooks = Record<string, never>;
export interface components {
    schemas: {
        AddMemberRequest: {
            /** @description Email of an active account. */
            email: string;
            /** @description `admin` or `member`. */
            role: string;
        };
        AskRequest: {
            /**
             * @description Choices for the question, empty the first time. To change an assumption or answer a clarification, send the
             *     same question with `choices` of the previous answer, the changed entry replaced or added. A question whose text
             *     was edited is a new question: send `choices` empty. The backend does not compare with earlier questions; it
             *     applies the choices it recognizes for this question and ignores the others with a warning
             *     `choice_not_applied`. Choices not listed use their default.
             */
            choices?: components["schemas"]["ChosenOption"][];
            /**
             * Format: uuid
             * @description Conversation to continue, from `AskResponse.conversation_id` of the previous question; the model then reads the
             *     question as a follow-up of the last turns. Leave out to start a new conversation. A conversation of another
             *     user or another dataset is 404.
             */
            conversation_id?: string | null;
            question: string;
        };
        /** @description Answer to one question, with the evidence behind it. See `AskStatus` for which fields are set. */
        AskResponse: {
            /**
             * @description Short answer in Vietnamese for an `answered` question that is not a "why" question and has at least one row:
             *     written by the language model from the first 10 rows of the result, every number checked against those rows,
             *     or a template sentence written by the backend. Null otherwise; a "why" question has `explanation.summary`.
             */
            answer_text: string | null;
            /**
             * @description True when `answer_text` was written by the language model, false for the template sentence (the model failed,
             *     timed out, wrote a number that is not in the rows, or `ANSWER_TEXT=false`). Null when `answer_text` is null.
             */
            answer_text_by_model: boolean | null;
            chart: components["schemas"]["ChartSuggestion"] | null;
            /**
             * @description Every choice this answer used, defaults included, in the shape of `AskRequest.choices`. To ask again with one
             *     choice changed, send this list with that entry replaced; for `needs_choice`, send it plus a pick for each
             *     clarification. Choices the backend did not recognize are not in it.
             */
            choices: components["schemas"]["ChosenOption"][];
            /** @description Questions back to the user, only for `needs_choice`. Every entry has `selected` null. */
            clarifications: components["schemas"]["Choice"][];
            /**
             * Format: uuid
             * @description Conversation this question belongs to (`AskRequest.conversation_id`, or a new one when it was empty). Send it
             *     back with the next question so the model reads it as a follow-up.
             */
            conversation_id: string;
            explanation: components["schemas"]["Explanation"] | null;
            /**
             * @description Text for the end user (Vietnamese): the reason for `rejected` and `failed`, the prompt for `needs_choice`;
             *     null for `answered`.
             */
            message: string | null;
            /** @description Language model configured for the question, for example `gemini-2.5-flash` or `mock`. */
            model: string;
            /**
             * Format: uuid
             * @description Id of the question in the history (`queries`); null for `needs_choice`.
             */
            query_id: string | null;
            /** @description The question exactly as sent in `AskRequest.question`. */
            question: string;
            result: components["schemas"]["ResultTable"] | null;
            /** @description SQL written by the model, on logical table names. After a fix round, the fixed SQL. */
            sql_generated: string | null;
            /**
             * @description SQL that ran: the generated SQL after the guard added row-level filters and a row limit. Still on logical table
             *     names, no file paths.
             */
            sql_guarded: string | null;
            status: components["schemas"]["AskStatus"];
            /**
             * @description What the backend did, in order, with timings. A step may appear twice, for example `run` before and after a
             *     `repair`.
             */
            steps: components["schemas"]["AskStep"][];
            understanding: components["schemas"]["Understanding"] | null;
            /**
             * Format: uuid
             * @description Dataset version the question ran on.
             */
            version_id: string;
            /** @description Notes shown next to the result; they never hide it. */
            warnings: components["schemas"]["Warning"][];
        };
        /**
         * @description Outcome of the question, and which fields of `AskResponse` are set:
         *     - `answered`: the SQL ran. Set: `query_id`, `sql_generated`, `sql_guarded`, `understanding`, `result`; `chart`
         *       when a chart fits; `explanation` for a "why" question (when the breakdown cannot be made, a warning
         *       `explanation_unavailable` says why).
         *     - `needs_choice`: the question can be read several ways; pick one option of each entry in `clarifications` and ask
         *       again. This can happen before the model is called (a word matches several columns) or after (the two slices of
         *       a "why" question cannot be told). Set: `message`, `clarifications`; `sql_generated` when the model was called.
         *       Not recorded in the history, so `query_id` is null.
         *     - `rejected`: the guard refused the SQL the model wrote in the `repair` round (when the guard refuses the model's
         *       first SQL, for any reason, the question always gets that round), or the SQL the server built for a "why"
         *       breakdown. Set: `message`, `query_id`, `sql_generated`. For the model's SQL, the `guard` step that stopped it has
         *       `ok` false and the guard's reason in `detail`, and `message` is a fixed text asking to rephrase the question, since
         *       the user cannot fix the SQL; for a breakdown, `message` gives the guard's reason. Also when
         *       the model finds that no column of the dataset holds the information the question needs (a rating that is not
         *       there, a forecast): no SQL runs, `sql_generated` is null, and `message` gives the reason and the closest columns.
         *     - `failed`: the SQL could not run and the one fix round did not help, either because the fixed SQL failed too or
         *       because the model failed during the fix; or the SQL ran past the time or memory limit, which gets no fix round.
         *       Set: `message`, `query_id`, `sql_generated`; `sql_guarded` unless that SQL never passed the guard. The `run` or
         *       `repair` step that stopped it has `ok` false.
         *
         *     A fix round starts when the analytics engine fails on the SQL, when the guard refuses it for any reason, or when the
         *     model's reply cannot be read; there is at most one round per question, and the fixed SQL goes through the guard
         *     again. The status names the step that stopped the question: the guard gives `rejected`, the engine or the fix round
         *     gives `failed`. A model error before any SQL exists is not a status: the request gets 502 or 503.
         * @enum {string}
         */
        AskStatus: "answered" | "needs_choice" | "rejected" | "failed";
        AskStep: {
            /** @description Text for the end user (Vietnamese), for example the guard's reason or the engine error. */
            detail: string | null;
            /** Format: int64 */
            duration_ms: number;
            name: components["schemas"]["StepName"];
            /**
             * @description False for a step that failed. A failed step does not always stop the question: a failed `run` or `guard` can be
             *     followed by `repair`, a failed `narrate` by a template summary. `AskStatus` tells whether the question stopped.
             */
            ok: boolean;
        };
        AssignAdminRequest: {
            /** @description Email of an active account; it becomes `admin` of the workspace. */
            email: string;
        };
        /** @description One group of a breakdown table in one slice. */
        BreakdownKey: {
            /** @description `DimensionBreakdown.column`: a column name of the dataset, not of `result.columns`. */
            column: string;
            slice: components["schemas"]["SliceName"];
            value: components["schemas"]["CellValue"] | null;
        };
        /** @description One cell: a JSON number, string or boolean; read it with the `kind` of its column. */
        CellValue: number | string | boolean;
        ChangePasswordRequest: {
            current_password: string;
            /** @description 8 to 128 characters. */
            new_password: string;
        };
        /**
         * @description Kind of chart:
         *     - `bar`: horizontal bars, one per group of `x`, largest first; compares groups.
         *     - `column`: vertical columns over the periods of `x`; up to 10 periods.
         *     - `line`: a line with a marker on each period of `x`; more than 10 periods, or one line per group of `series`.
         *     - `stacked_column`: vertical columns over the periods of `x`, each split into the groups of `series`.
         * @enum {string}
         */
        ChartKind: "bar" | "column" | "line" | "stacked_column";
        /**
         * @description How the measures in `y` are drawn:
         *     - `single`: one measure.
         *     - `grouped`: side by side on one scale, because the measures share a unit.
         *     - `panels`: one small chart per measure sharing the axis of `x`, because the units differ or are unknown; two
         *       scales in one chart are not used, since choosing them can make the same numbers look like anything.
         * @enum {string}
         */
        ChartLayout: "single" | "grouped" | "panels";
        /**
         * @description Suggested chart; null in `AskResponse` when the result is better shown as a table only (one value, too many
         *     groups, more than one grouping column without time). Every name is a name from `result.columns`.
         */
        ChartSuggestion: {
            kind: components["schemas"]["ChartKind"];
            layout: components["schemas"]["ChartLayout"];
            /**
             * @description The groups after `top` can be summed into one entry labelled "Khác": every measure is a sum or a plain count,
             *     and `result` holds every row. False: they are left out, and the chart says how many.
             */
            other_bucket: boolean;
            /**
             * @description Measures in `y` that are not totals (averages, ratios, minimum, maximum): drawn as narrow columns, and never
             *     summed into a "Khác" entry.
             */
            ratio: string[];
            /**
             * @description For `stacked_column`, and for `line` with one line per group: the group column whose values become the parts
             *     or the lines. Null otherwise.
             */
            series: string | null;
            /**
             * Format: int32
             * @description Most groups to draw, largest first: bars for `bar`, parts for `stacked_column`. Null when every group is drawn.
             */
            top: number | null;
            /** @description Category axis: a group column for `bar`, a time column for the other kinds. */
            x: string;
            /** @description Measures to draw, at most 4. */
            y: string[];
        };
        /**
         * @description A point where the question can be read several ways. To change it, ask the same question again with
         *     `AskResponse.choices`, the entry for this `id` replaced.
         */
        Choice: {
            /**
             * @description Id set by the backend; send it back unchanged. Stable for the same question on the same dataset version: made
             *     from the kind of choice and what it is about (a word and its candidate columns, the period to compare with),
             *     never from the text of the SQL, so a pick still applies when the model writes different SQL.
             */
            id: string;
            /** @description Text for the end user (Vietnamese): what can be read several ways, for example which column a word means. */
            label: string;
            options: components["schemas"]["ChoiceOption"][];
            /** @description Id of the option in use; null when the user has to pick (`needs_choice`). */
            selected: string | null;
        };
        ChoiceOption: {
            /** @description Opaque id set by the backend; send it back unchanged. */
            id: string;
            /** @description Text for the end user (Vietnamese). */
            label: string;
        };
        /**
         * @description The pick for one `Choice`. A `choice_id` or `option_id` the backend does not recognize (for example a choice that
         *     no longer applies) is ignored, left out of `AskResponse.choices`, and reported with a warning `choice_not_applied`:
         *     an assumption falls back to its default, a clarification is asked again.
         */
        ChosenOption: {
            /** @description `Choice.id` from an earlier answer. */
            choice_id: string;
            /** @description One of the `ChoiceOption.id` of that choice. */
            option_id: string;
        };
        Column: {
            /** @description DuckDB type, for example `BIGINT`, `DOUBLE`, `VARCHAR`, `DATE`. */
            data_type: string;
            name: string;
        };
        /**
         * @description What a result column is:
         *     - `dimension`: a grouping key that is not time, for example a market.
         *     - `time`: a grouping key on time, for example a quarter.
         *     - `measure`: an aggregate, for example a sum; its cells can be drilled into.
         *     - `other`: anything else, for example a column of a query without aggregates.
         * @enum {string}
         */
        ColumnRole: "dimension" | "time" | "measure" | "other";
        /**
         * @description - `periods`: the slices are two periods; `compare` is the later one.
         *     - `groups`: the slices are two values of one column, with no order in time.
         * @enum {string}
         */
        Comparison: "periods" | "groups";
        /** @description One conversation on a dataset. */
        ConversationItem: {
            /** Format: date-time */
            created_at: string;
            /** Format: uuid */
            id: string;
            /** @description The first question of the conversation, shortened. */
            title: string;
        };
        /** @description Conversations of the calling user on a dataset, newest first. */
        ConversationListResponse: {
            conversations: components["schemas"]["ConversationItem"][];
        };
        /** @description A conversation and its recorded turns, oldest first. */
        ConversationResponse: {
            conversation: components["schemas"]["ConversationItem"];
            turns: components["schemas"]["TurnItem"][];
        };
        CreateSourceRequest: {
            database: string;
            /** @description Host name or IP; must be listed in SOURCE_ALLOWED_HOSTS. */
            host: string;
            /** @description Stored encrypted; never returned. */
            password: string;
            /**
             * Format: int32
             * @description Defaults to 5432.
             */
            port?: number | null;
            /** @description `disable` or `prefer` (default). TLS-only servers are not supported yet. */
            sslmode?: string | null;
            user: string;
        };
        CreateUserRequest: {
            display_name?: string;
            email: string;
            /** @description 8 to 128 characters. */
            password: string;
        };
        CreateWorkspaceRequest: {
            /** @description 1 to 100 characters. The slug is derived from it. */
            name: string;
        };
        Dataset: {
            /** Format: date-time */
            created_at: string;
            /**
             * @description Profile status of the current version (`pending`, `structure`, `semantic`, `failed`); null while the dataset
             *     has no current version. The UI polls it to know when the semantic profile is done.
             */
            current_profile_status: string | null;
            current_source_model: components["schemas"]["SourceModel"] | null;
            /**
             * Format: date-time
             * @description When the current version was created: upload time, or the last snapshot of a database source.
             */
            current_version_at: string | null;
            /** Format: uuid */
            current_version_id: string | null;
            domain: string | null;
            /** Format: uuid */
            id: string;
            /** @description Table name used in SQL, derived from the dataset name. */
            logical_table: string;
            name: string;
            /**
             * @description True when the current version stays at `structure` because the semantic profile was already tried with the
             *     running LLM provider and its answer was not usable: no suggested questions will come until the provider
             *     changes. Asking still works.
             */
            semantic_unavailable: boolean;
            /**
             * Format: uuid
             * @description Database source the dataset was snapshotted from; null for uploaded files.
             */
            source_id: string | null;
            /** @description Kind of the source (`local_file` or `postgres`); null right after creation. */
            source_kind: string | null;
            /** @description `schema.table` snapshotted from a database source; null for uploaded files. */
            source_table: string | null;
            /**
             * @description Suggested questions of the current version, written by the semantic profile; empty while that step has not
             *     finished.
             */
            suggested_questions: string[];
        };
        DatasetVersion: {
            /** Format: date-time */
            created_at: string;
            /** Format: uuid */
            dataset_id: string;
            file_hash: string | null;
            /** Format: uuid */
            id: string;
            parquet_path: string | null;
            profile_status: string;
            /** Format: int64 */
            row_count: number | null;
            source_kind: string;
            /** Format: int32 */
            version: number;
        };
        /** @description Contribution of every group of one dimension; the contributions add up to `change`. */
        DimensionBreakdown: {
            /** @description Dimension column, as named in the dataset (not a name from `result.columns`). */
            column: string;
            /** @description Every group of the column, largest contribution (by absolute value) first. */
            groups: components["schemas"]["GroupContribution"][];
            /** @description How to read the group values, like the `kind` of a result column. */
            kind: components["schemas"]["ValueKind"];
            /** @description Vietnamese name of the column from the column profile; null when the profile has none. */
            label: string | null;
            /** @description Ratio measures only: sum of `mix_effect` of every group; `mix_effect + rate_effect` is `change`. */
            mix_effect: string | null;
            /** @description Ratio measures only: sum of `rate_effect` of every group. */
            rate_effect: string | null;
            /** @description SQL that computed the groups of both slices, after the guard. */
            sql: string;
        };
        /** @description One grouping key of the clicked cell. */
        DrillKey: {
            /** @description A name from `result.columns`, of a column with role `dimension` or `time`. */
            column: string;
            value: components["schemas"]["CellValue"] | null;
        };
        /**
         * @description Which number to trace back to its source rows: set exactly one of `cell` and `breakdown`. Rows come one page at a
         *     time, in the order the analytics engine reads them.
         */
        DrillRequest: {
            breakdown?: components["schemas"]["BreakdownKey"] | null;
            /**
             * @description A cell of `AskResponse.result`: the values of every `dimension` and `time` column of its row, one key per
             *     column. An empty list is the one row of a query without grouping keys.
             */
            cell?: components["schemas"]["DrillKey"][] | null;
            /**
             * Format: int32
             * @description Rows per page, 1 to 1000. Default 100.
             */
            limit?: number;
            /**
             * Format: int64
             * @description Number of source rows to skip. Default 0.
             */
            offset?: number;
            /**
             * Format: uuid
             * @description `AskResponse.query_id` of the answer the number belongs to.
             */
            query_id: string;
        };
        /** @description One page of the source rows behind a number. */
        DrillResponse: {
            /**
             * @description The page: every column of the dataset, including `_source_file`, the file each row came from. Here
             *     `result.total_rows` counts the rows of this page only.
             */
            result: components["schemas"]["ResultTable"];
            /**
             * @description The SQL that read this page, after the guard: the filters of the answered question, the keys of the clicked
             *     number, and the row filters of the member asking now (which may differ from the member who asked the question).
             */
            sql: string;
            /**
             * Format: int64
             * @description Number of source rows behind the number, over all pages.
             */
            total_rows: number;
        };
        /** @description Error body returned by every endpoint on failure. */
        ErrorBody: {
            code: components["schemas"]["ErrorCode"];
            /** @description Human-readable explanation in Vietnamese, meant to be shown to the end user as is. */
            message: string;
        };
        /**
         * @description Stable machine-readable error code; one per HTTP status.
         * @enum {string}
         */
        ErrorCode: "bad_request" | "unauthorized" | "forbidden" | "not_found" | "too_large" | "internal" | "not_implemented" | "upstream" | "unavailable";
        /**
         * @description Breakdown of a change between two slices (two periods or two groups); null in `AskResponse` unless an answered
         *     "why" question has one. Numbers are exact strings of decimal digits.
         */
        Explanation: {
            /** @description Slice compared against. */
            base: components["schemas"]["ExplanationSlice"];
            /** @description `compare.value - base.value`. */
            change: string;
            /** @description Slice being explained. */
            compare: components["schemas"]["ExplanationSlice"];
            /**
             * @description What the two slices are, which decides the words for the change: up or down between two periods, higher or
             *     lower between two groups.
             */
            comparison: components["schemas"]["Comparison"];
            /** @description True when the measure counts rows (a count, not a sum): the measure is then itself the size of each slice. */
            counts_rows: boolean;
            /** @description One breakdown per dimension, the dimension that explains the change best first. */
            dimensions: components["schemas"]["DimensionBreakdown"][];
            /** @description Measure that changed, text for the end user (Vietnamese). */
            measure: string;
            /** @description How the measure adds up over groups, which decides how to read the numbers below. */
            measure_kind: components["schemas"]["MeasureKind"];
            /** @description `change / base.value * 100`, rounded to one decimal; null when `base.value` is zero or negative. */
            percent_change: string | null;
            /** @description Column whose two values are compared, for `groups`; null for `periods`. */
            slice_column: string | null;
            /** @description Vietnamese name of `slice_column` from the column profile; null when the profile has none. */
            slice_column_label: string | null;
            /** @description Plain-language summary for the end user (Vietnamese). Every number in it appears in this breakdown. */
            summary: string;
            /**
             * @description False when the model's text failed the number check, or the model was not reached, and `summary` is a template
             *     sentence written by the backend.
             */
            summary_by_model: boolean;
            /**
             * @description Points under the main answer, written by the backend from the numbers above; each is said only when the numbers
             *     support it.
             */
            summary_points: components["schemas"]["SummaryPoint"][];
            /** @description Main answer of the summary, one sentence written by the backend from the numbers above (Vietnamese). */
            summary_title: string;
            volume_price: components["schemas"]["VolumePrice"] | null;
        };
        /** @description One of the two slices. */
        ExplanationSlice: {
            /** @description SQL condition that selects the rows of the slice, on logical names. */
            filter: string;
            /** @description Text for the end user (Vietnamese) naming the slice, for example a quarter. */
            label: string;
            /** @description Number of rows of the slice. */
            rows: string;
            /** @description Value of the measure on the slice. */
            value: string;
        };
        /** @description One group of a dimension. `value` is null for the group of rows where the dimension column is null. */
        GroupContribution: {
            base_value: string;
            compare_value: string;
            /**
             * @description Part of `change` from this group: `compare_value - base_value` for a total, `mix_effect + rate_effect` for a
             *     ratio.
             */
            contribution: string;
            /**
             * @description Ratio measures only: part of `change` because this group's share of the denominator changed,
             *     `(compare_share - base_share) * (base_value + compare_value) / 2`. For a group with no denominator in one slice,
             *     the ratio of that slice is taken equal to the other slice, so the whole contribution is a mix effect.
             */
            mix_effect: string | null;
            /**
             * @description Ratio measures only: part of `change` because the ratio inside this group changed,
             *     `(base_share + compare_share) / 2 * (compare_value - base_value)`.
             */
            rate_effect: string | null;
            value: components["schemas"]["CellValue"] | null;
        };
        HealthResponse: {
            /** @description Always `ok` when the server answers. */
            status: string;
        };
        /** @description Multipart form for listing the sheets of an Excel file. The file is not stored. */
        ListSheetsRequest: {
            /**
             * Format: binary
             * @description One Excel `.xlsx` file.
             */
            file: string;
        };
        LoginRequest: {
            email: string;
            password: string;
        };
        MeResponse: {
            user: components["schemas"]["User"];
            /** @description Workspaces the user belongs to, with their role in each, sorted by name. */
            workspaces: components["schemas"]["Membership"][];
        };
        /**
         * @description - `total`: a sum or a count; group values add up to the slice value, a contribution is `compare_value - base_value`.
         *     - `ratio`: a ratio or an average (numerator over denominator); slice and group values are ratios rounded to a fixed
         *       number of decimals, a contribution is `mix_effect + rate_effect`.
         * @enum {string}
         */
        MeasureKind: "total" | "ratio";
        /** @description A member of a workspace. */
        MemberRow: {
            display_name: string;
            email: string;
            /** @description `admin` or `member`. */
            role: string;
            /** Format: uuid */
            user_id: string;
        };
        /** @description A workspace the user belongs to, with their role in it. */
        Membership: {
            name: string;
            /** @description `admin` or `member`. */
            role: string;
            /** @description Used in URLs: `/workspaces/{slug}/...`. */
            slug: string;
            /** Format: uuid */
            workspace_id: string;
        };
        /** @description One many-to-one join of a snapshot. */
        ModelJoin: {
            /** @description Dataset columns copied from `to_table`, as named in the dataset. */
            columns: string[];
            /** @description Foreign-key constraint name in the source database. */
            constraint: string;
            from_columns: string[];
            /** @description `schema.table` holding the foreign key: the main table, or a table joined before (two hops). */
            from_table: string;
            /** @description Dataset columns that identify one row of `to_table` (the foreign-key values pointing to it). */
            key_columns?: string[];
            /** @description Prefix of the dataset columns copied from `to_table`: a column `c` of it is named `<prefix>.c`. */
            prefix: string;
            /** @description Relation id from the relation list; a snapshot again resolves the same ids. */
            relation: string;
            to_columns: string[];
            /** @description `schema.table` the foreign key points to. */
            to_table: string;
        };
        RelationsRequest: {
            /** @description `schema.table` from the table list. */
            table: string;
        };
        RelationsResponse: {
            /** @description Readable columns of the chosen table. */
            column_count: number;
            /** @description Foreign keys leaving the chosen table (depth 1) or a table joined at depth 1 (depth 2), in join order. */
            many_to_one: components["schemas"]["SourceRelation"][];
            /** @description Foreign keys of other tables pointing into the chosen table: never joined, they would repeat its rows. */
            one_to_many: components["schemas"]["SourceRelation"][];
            table: string;
        };
        ResetPasswordResponse: {
            /** @description Shown once to hand to the user; not stored anywhere in plain text. */
            temporary_password: string;
        };
        ResultColumn: {
            kind: components["schemas"]["ValueKind"];
            /**
             * @description Column name as the SQL returns it, made unique: from left to right, a name already used gets the smallest
             *     suffix `_2`, `_3`, ... that no column has, so `a, a, a_2` becomes `a, a_3, a_2`.
             */
            name: string;
            role: components["schemas"]["ColumnRole"];
        };
        /** @description Rows of the result; null in `AskResponse` unless the status is `answered`. At most 1000 rows are sent. */
        ResultTable: {
            columns: components["schemas"]["ResultColumn"][];
            /**
             * @description True when `total_rows` equals the guard's row cap (whether the guard added the LIMIT or lowered one the SQL
             *     had): the full result may have more rows, so show
             *     `total_rows` as "at least".
             */
            limit_reached: boolean;
            /** @description One array per row, cells in the order of `columns`; null is a SQL NULL. */
            rows: (components["schemas"]["CellValue"] | null)[][];
            /**
             * Format: int64
             * @description Number of rows `sql_guarded` returned. This is the full result unless `limit_reached`.
             */
            total_rows: number;
            /** @description True when `rows` holds fewer rows than `total_rows`. */
            truncated: boolean;
        };
        SetRoleRequest: {
            /** @description `admin` or `member`. */
            role: string;
        };
        SetupRequest: {
            display_name?: string;
            email: string;
            /** @description 8 to 128 characters. */
            password: string;
            /** @description Code printed in the backend log at startup; case and surrounding spaces are ignored. */
            setup_code: string;
            /** @description Name of the first workspace; empty gives a default name. */
            workspace_name?: string;
        };
        SetupStatusResponse: {
            /** @description True while the installation has no account yet: show the first-run setup screen instead of sign-in. */
            needed: boolean;
        };
        /** @description A sheet of an Excel file, as shown when choosing sheets to upload. */
        SheetSummary: {
            /**
             * Format: int64
             * @description Rows below the header, from the data range the file records. An estimate: may include empty rows, 0 when the
             *     file does not record it.
             */
            approx_rows: number;
            /** @description Column names from the header row, normalised as on upload; empty if the sheet has no data. */
            columns: string[];
            /** @description Hidden in Excel. Hidden sheets are skipped when no sheet is chosen. */
            hidden: boolean;
            name: string;
        };
        /**
         * @description `base` or `compare`: one of the two slices of `AskResponse.explanation`.
         * @enum {string}
         */
        SliceName: "base" | "compare";
        SnapshotRequest: {
            /**
             * Format: uuid
             * @description Snapshot again into this dataset as a new version. The joins of the current version are reused.
             */
            dataset_id?: string | null;
            /** @description Name of the new dataset; defaults to the table name. */
            name?: string | null;
            /** @description Ids of many-to-one relations to join (from the relation list of the table). Not allowed with `dataset_id`. */
            relations?: string[];
            /** @description `schema.table` from the table list. Not needed with `dataset_id`: the table snapshotted before is used. */
            table?: string | null;
        };
        /**
         * @description How a dataset version snapshotted from a database was built: the main (fact) table and the tables joined
         *     many-to-one along foreign keys.
         */
        SourceModel: {
            /**
             * Format: int64
             * @description Rows of the main table and of the joined result, counted in the same snapshot; always equal (a snapshot whose
             *     join would change the row count is refused).
             */
            fact_rows: number;
            /** @description `schema.table` whose rows become the rows of the dataset. */
            fact_table: string;
            /** Format: int64 */
            joined_rows: number;
            /** @description Joins in the order they appear in the SQL; empty when only the main table was copied. */
            joins: components["schemas"]["ModelJoin"][];
        };
        /** @description One foreign key around the chosen table, as the UI lists it. */
        SourceRelation: {
            /** @description Columns the join would add (the target table's readable columns minus the key columns). */
            column_count: number;
            constraint: string;
            /**
             * Format: int32
             * @description 1 for a foreign key of the chosen table, 2 for a foreign key of a table joined at depth 1.
             */
            depth: number;
            from_columns: string[];
            from_table: string;
            /** @description Stable id to send back in the snapshot request: `schema.table/constraint`, two hops joined with `>`. */
            id: string;
            /** @description True when the join keeps one row per row of the chosen table and the account can read the target. */
            joinable: boolean;
            /** @description Id of the depth-1 relation this one continues; null at depth 1. */
            parent: string | null;
            /** @description Why the relation is not joined, in Vietnamese; null when joinable. */
            reason: string | null;
            to_columns: string[];
            to_table: string;
        };
        /** @description A database source without its password. */
        SourceResponse: {
            /** Format: date-time */
            created_at: string;
            database: string;
            host: string;
            /** Format: uuid */
            id: string;
            kind: string;
            /** Format: int32 */
            port: number;
            sslmode: string;
            user: string;
        };
        SourceTable: {
            /** @description `table` or `view`. */
            kind: string;
            name: string;
            schema: string;
        };
        /**
         * @description Step of the backend:
         *     - `context`: read the dataset profile and build the prompt.
         *     - `llm`: the language model writes SQL.
         *     - `guard`: check the SQL, add row-level filters and a row limit.
         *     - `run`: run the SQL on the analytics engine.
         *     - `repair`: the language model fixes SQL that failed to run, or that the guard refused because a table or column
         *       does not exist or because it could build a value too large (at most once).
         *     - `explain`: compute the breakdown of a "why" question (code, no model).
         *     - `narrate`: the language model writes the summary of the breakdown, or the `answer_text` of a plain question.
         *     - `record`: save the question in the history.
         * @enum {string}
         */
        StepName: "context" | "llm" | "guard" | "run" | "repair" | "explain" | "narrate" | "record";
        /** @description One point of the summary: a short lead (shown in bold) and the sentence after it. */
        SummaryPoint: {
            kind: components["schemas"]["SummaryPointKind"];
            /** @description Lead of the point (Vietnamese), for example "Lý do chính". */
            lead: string;
            /** @description Rest of the point (Vietnamese). */
            text: string;
        };
        /**
         * @description - `reason`: what mostly makes the change (the size of the slices, the volume and the price, or the mix and the
         *       rate).
         *     - `outliers`: groups that changed much more or much less than the whole, or exist on one side only.
         *     - `even`: no group stands out; the largest contributions are the largest groups.
         *     - `largest`: the group with the largest contribution in the direction of the change, when the numbers do not allow
         *       comparing groups with the whole (a base slice of zero or less, or a change too close to zero).
         *     - `against`: a group that moved the other way.
         *     - `dropped`: dimensions the summary leaves out and why; they stay in `Explanation.dimensions`.
         * @enum {string}
         */
        SummaryPointKind: "reason" | "outliers" | "even" | "largest" | "against" | "dropped";
        TestSourceResponse: {
            /** @description Number of tables and views the account can read. */
            table_count: number;
        };
        /** @description One recorded question of a conversation. Questions waiting for a choice (`needs_choice`) are not recorded. */
        TurnItem: {
            /** Format: date-time */
            created_at: string;
            /**
             * Format: uuid
             * @description Id of the question in the history; use it to trace source rows like a single question.
             */
            query_id: string;
            question: string;
            /** @description SQL that ran (`sql_guarded`); null when the guard refused it. */
            sql: string | null;
            /** @description `answered`, `rejected` or `failed`. */
            status: components["schemas"]["AskStatus"];
            /** @description Short text of the "understood as" card (measures, filters, grouping, period); null unless answered. */
            understood: string | null;
        };
        /**
         * @description The "understood as" card, built from `sql_guarded`: what is measured, filtered, grouped, sorted, over which
         *     period, and the assumptions made. Null in `AskResponse` unless the status is `answered`. `limit` is null when only
         *     the guard's row cap applies; `time_range` is null when the SQL has no time filter.
         */
        Understanding: {
            /** @description Assumptions the user can change, including clarifications answered earlier; every entry has `selected` set. */
            assumptions: components["schemas"]["Choice"][];
            /**
             * @description Filters of the question, then the row-level filters the guard added for the user's data role (the label says
             *     which).
             */
            filters: components["schemas"]["UnderstoodItem"][];
            group_by: components["schemas"]["UnderstoodItem"][];
            limit: components["schemas"]["UnderstoodItem"] | null;
            measures: components["schemas"]["UnderstoodItem"][];
            order_by: components["schemas"]["UnderstoodItem"][];
            time_range: components["schemas"]["UnderstoodItem"] | null;
        };
        /** @description One thing the question was understood to mean. */
        UnderstoodItem: {
            /**
             * @description True when the item is carried over from the previous turn of the conversation: the same as in that turn's card
             *     (same SQL fragment or same label) and not named in this question. Omitted when false.
             */
            from_previous?: boolean;
            /** @description Text for the end user (Vietnamese) naming the measure, filter, grouping, order or period. */
            label: string;
            /** @description Fragment of `sql_guarded` this item comes from. */
            sql: string;
        };
        /** @description Multipart form for uploading a dataset. Total size is limited by MAX_UPLOAD_MB (413 above it). */
        UploadDatasetRequest: {
            /** @description Add a new version to this dataset instead of creating a new one. */
            dataset_id?: string | null;
            /**
             * @description One or more CSV or Parquet files (repeat the field), or one Excel `.xlsx` file. Files with the same columns are
             *     stacked into one table.
             */
            file: string[];
            /** @description Name of the new dataset; defaults to the first file name. */
            name?: string | null;
            /**
             * @description Sheet of the Excel file to upload; repeat to stack several sheets with the same columns. Not needed when the
             *     file has one sheet.
             */
            sheet?: string[] | null;
        };
        UploadResult: {
            /** @description Columns of the new table; empty when `created` is false. */
            columns: components["schemas"]["Column"][];
            /** @description False when the files match an existing version: nothing new was created. */
            created: boolean;
            dataset: components["schemas"]["Dataset"];
            version: components["schemas"]["DatasetVersion"];
        };
        User: {
            display_name: string;
            email: string;
            /** Format: uuid */
            id: string;
            is_system_admin: boolean;
        };
        /** @description An account in the system administrator's list. */
        UserSummary: {
            /** Format: date-time */
            created_at: string;
            display_name: string;
            email: string;
            /** Format: uuid */
            id: string;
            is_active: boolean;
            is_system_admin: boolean;
        };
        /**
         * @description How to read the cells of a column, decided by the column type. Any cell may also be null (SQL NULL).
         *     - `text`: JSON string.
         *     - `number`: JSON number, for types a JavaScript number holds exactly: integers up to 32 bits, FLOAT, DOUBLE. NaN
         *       and infinity are sent as null.
         *     - `decimal`: JSON string of decimal digits, integer or not, for example "5000" or "-1234.50", for BIGINT
         *       (COUNT returns one), HUGEINT, unsigned 64 and 128 bit integers and DECIMAL, which a JavaScript number would
         *       round. Convert with `Number()` only to draw a chart or to sort; comparing the strings sorts "10" before "9".
         *     - `boolean`: JSON boolean.
         *     - `date`: JSON string `YYYY-MM-DD`.
         *     - `timestamp`: JSON string in ISO 8601, `YYYY-MM-DDTHH:MM:SS[.ffffff]` for TIMESTAMP (no time zone, as stored),
         *       ending in `Z` for TIMESTAMPTZ (converted to UTC).
         *     - `other`: JSON string as the analytics engine prints the value: TIME, INTERVAL, lists, structs and other types.
         *
         *     A `date` or `timestamp` with a year after 9999 or before 1 AD uses the ISO 8601 expanded form: `+10000-01-01`;
         *     year 1 BC is `0000`, 2 BC is `-0001`. Infinite dates and times are sent as null. A result column of type TIME_NS,
         *     TIME WITH TIME ZONE, BIT, BIGNUM, VARIANT, GEOMETRY or a type the backend does not recognize is not read: the `run`
         *     step fails and the fix round casts it to text.
         * @enum {string}
         */
        ValueKind: "text" | "number" | "decimal" | "boolean" | "date" | "timestamp" | "other";
        VersionResponse: {
            /** @description Language model provider in use, for example `mock` or `gemini-2.5-flash`. */
            llm: string;
            name: string;
            version: string;
        };
        /**
         * @description Change split into a volume effect and a price effect, for a measure that is volume times price (for example
         *     orders times average order value); null in `Explanation` for other measures.
         */
        VolumePrice: {
            /** @description Measure divided by volume, rounded to a fixed number of decimals; the only rounded numbers here. */
            base_price: string;
            base_volume: string;
            compare_price: string;
            compare_volume: string;
            /** @description Value of the measure if the volume had stayed at `base_volume`: `base_volume * compare_price`. */
            if_volume_unchanged: string;
            /** @description `change - volume_effect`, so that the two effects add up to `change` exactly. */
            price_effect: string;
            /** @description Text for the end user (Vietnamese) naming the price, for example the average order value. */
            price_label: string;
            /** @description SQL that computed the volume and the measure of both slices, after the guard. */
            sql: string;
            /** @description `(compare_volume - base_volume) * (base_price + compare_price) / 2`, exact from the rounded prices. */
            volume_effect: string;
            /** @description Text for the end user (Vietnamese) naming the volume, for example the number of orders. */
            volume_label: string;
        };
        Warning: {
            code: components["schemas"]["WarningCode"];
            /** @description Text for the end user (Vietnamese). */
            message: string;
        };
        /**
         * @description Kind of note:
         *     - `empty_result`: no rows.
         *     - `all_null`: a result column holds only nulls.
         *     - `filter_value_not_found`: the SQL filters on a value that does not occur in the column.
         *     - `unusual_magnitude`: a value is far outside what the data makes likely, for example a ratio above 100%.
         *     - `explanation_unavailable`: a "why" question was answered without a breakdown; the message says why.
         *     - `choice_not_applied`: a choice sent with the question was not recognized and the default was used.
         *     - `premise_mismatch`: a "why" question assumes a direction (a drop, a rise) that the computed change does not have;
         *       the breakdown explains the actual change.
         * @enum {string}
         */
        WarningCode: "empty_result" | "all_null" | "filter_value_not_found" | "unusual_magnitude" | "explanation_unavailable" | "choice_not_applied" | "premise_mismatch";
        /** @description A workspace as the system administrator sees it: name and members, none of its data. */
        WorkspaceOverview: {
            /** Format: date-time */
            created_at: string;
            members: components["schemas"]["MemberRow"][];
            name: string;
            slug: string;
            /** Format: uuid */
            workspace_id: string;
        };
    };
    responses: never;
    parameters: never;
    requestBodies: never;
    headers: never;
    pathItems: never;
}
export type $defs = Record<string, never>;
export interface operations {
    listUsers: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description OK */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["UserSummary"][];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 401 not signed in, 403 not a system administrator */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    createUser: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateUserRequest"];
            };
        };
        responses: {
            /** @description Created */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["User"];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 invalid input or email taken, 401, 403 */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    activateUser: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Account id (UUID) */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Activated */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 invalid id, 401, 403, 404 */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    deactivateUser: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Account id (UUID) */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Deactivated */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 invalid id, own account or last system administrator, 401, 403, 404 */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    resetUserPassword: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Account id (UUID) */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description OK */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResetPasswordResponse"];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 invalid id, 401, 403, 404 */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    listAllWorkspaces: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description OK */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WorkspaceOverview"][];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 401 not signed in, 403 not a system administrator */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    assignWorkspaceAdmin: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace id (UUID) */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AssignAdminRequest"];
            };
        };
        responses: {
            /** @description Assigned */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 invalid id or email, 401, 403, 404 unknown workspace or account */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    demoLogin: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Not implemented yet */
            501: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    login: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["LoginRequest"];
            };
        };
        responses: {
            /** @description Signed in */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MeResponse"];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 invalid input, 401 wrong email or password */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    logout: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Signed out */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    me: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description OK */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MeResponse"];
                };
            };
            /** @description Not signed in or session expired */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    changePassword: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ChangePasswordRequest"];
            };
        };
        responses: {
            /** @description Password changed */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 wrong current password or invalid new password, 401 not signed in */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    setupStatus: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description OK */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SetupStatusResponse"];
                };
            };
        };
    };
    setup: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SetupRequest"];
            };
        };
        responses: {
            /** @description Signed in as the new administrator */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MeResponse"];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 invalid input, 403 wrong code or already set up */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    health: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description OK */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HealthResponse"];
                };
            };
        };
    };
    version: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description OK */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VersionResponse"];
                };
            };
        };
    };
    listWorkspaces: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description OK */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Membership"][];
                };
            };
            /** @description Not signed in */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    createWorkspace: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateWorkspaceRequest"];
            };
        };
        responses: {
            /** @description Created */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Membership"];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 invalid name, 401 not signed in */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    deleteWorkspace: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Deleted */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 401, 403 not admin, 404 not a member */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    renameWorkspace: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateWorkspaceRequest"];
            };
        };
        responses: {
            /** @description Renamed */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 invalid name, 401, 403 not admin, 404 not a member */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    listDatasets: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description OK */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Dataset"][];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 401, 404 not a member */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    uploadDataset: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "multipart/form-data": components["schemas"]["UploadDatasetRequest"];
            };
        };
        responses: {
            /** @description Same files as an existing version: nothing created */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["UploadResult"];
                };
            };
            /** @description New dataset or version created */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["UploadResult"];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 unreadable or mismatched files, 401, 403 not admin, 404 not a member or unknown dataset_id, 413 too large */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    listExcelSheets: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "multipart/form-data": components["schemas"]["ListSheetsRequest"];
            };
        };
        responses: {
            /** @description OK */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SheetSummary"][];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 not a readable Excel file, 401, 403 not admin, 404 not a member, 413 too large */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    getDataset: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Dataset id (UUID) */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description OK */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Dataset"];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 401, 404 not a member or no such dataset */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    deleteDataset: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Dataset id (UUID) */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Deleted */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 401, 403 not admin, 404 not a member or no such dataset */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    ask: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Dataset id (UUID) */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AskRequest"];
            };
        };
        responses: {
            /** @description Answer, or why there is none */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AskResponse"];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description The language model service failed or did not answer before any SQL was written */
            502: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description The language model is not configured on the server */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 empty question, invalid JSON body, dataset with no data or whose profile is not ready, or a question the mock model has no answer for; 401; 404 not a member, unknown dataset, or a conversation that is not the caller's */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    listConversations: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Dataset id (UUID) */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ConversationListResponse"];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 401; 404 not a member or invalid dataset id */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    getConversation: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Dataset id (UUID) */
                id: string;
                /** @description Conversation id (UUID) */
                conversation_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ConversationResponse"];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 401; 404 not a member, or no such conversation of the caller on this dataset */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    drill: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Dataset id (UUID) */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["DrillRequest"];
            };
        };
        responses: {
            /** @description One page of source rows and their total count */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DrillResponse"];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 invalid body, keys that do not match the result, a question that was rejected or failed, a question whose form cannot be traced yet (the message says why), a breakdown row of a question without that breakdown, or a deleted version; 401; 404 not a member, or no such question in this dataset */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    listMembers: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description OK */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MemberRow"][];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 401, 404 not a member */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    addMember: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AddMemberRequest"];
            };
        };
        responses: {
            /** @description Added */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 invalid role or already a member, 401, 403 not admin, 404 not a member or no active account with this email */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    removeMember: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Account id of the member */
                user_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Removed */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 last admin, 401, 403 not admin, 404 */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    setMemberRole: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Account id of the member */
                user_id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SetRoleRequest"];
            };
        };
        responses: {
            /** @description Changed */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 invalid role or last admin, 401, 403 not admin, 404 */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    listSources: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description OK */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SourceResponse"][];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 401, 403 not admin, 404 not a member */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    createSource: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateSourceRequest"];
            };
        };
        responses: {
            /** @description Created */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SourceResponse"];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description SOURCE_SECRET_KEY not set: database sources are off */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 invalid form, host not allowed or cannot connect, 401, 403 not admin, 404 not a member */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    deleteSource: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Source id */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Deleted */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 datasets still use it, 401, 403 not admin, 404 */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    listSourceRelations: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Source id */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RelationsRequest"];
            };
        };
        responses: {
            /** @description OK */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RelationsResponse"];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 unknown table or cannot connect, 401, 403 not admin, 404 */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    snapshotSource: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Source id */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SnapshotRequest"];
            };
        };
        responses: {
            /** @description Same content as an existing version: nothing created */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["UploadResult"];
                };
            };
            /** @description New dataset or version created */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["UploadResult"];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 unknown table or relation, cannot read it, or the joins change the row count, 401, 403 not admin, 404, 413 more rows than SOURCE_MAX_ROWS or bigger than MAX_UPLOAD_MB */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    listSourceTables: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Source id */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description OK */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SourceTable"][];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 cannot connect, 401, 403 not admin, 404 */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    testSource: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Source id */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Connected */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TestSourceResponse"];
                };
            };
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description 400 cannot connect, with how to fix it, 401, 403, 404 */
            "4XX": {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
}
