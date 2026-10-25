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
        delete?: never;
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
         * Ask a question about a dataset (not implemented yet: returns 501)
         * @description One JSON response, no streaming. A question that cannot be answered (`rejected`, `needs_choice`, `failed`) is still 200, with `status` and `message` in the body and the evidence gathered so far. Error codes are for invalid requests, access, and the language model service.
         */
        post: operations["ask"];
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
        /** Source rows behind a number (not implemented yet) */
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
            question: string;
        };
        /** @description Answer to one question, with the evidence behind it. See `AskStatus` for which fields are set. */
        AskResponse: {
            chart: components["schemas"]["ChartSuggestion"] | null;
            /**
             * @description Every choice this answer used, defaults included, in the shape of `AskRequest.choices`. To ask again with one
             *     choice changed, send this list with that entry replaced; for `needs_choice`, send it plus a pick for each
             *     clarification. Choices the backend did not recognize are not in it.
             */
            choices: components["schemas"]["ChosenOption"][];
            /** @description Questions back to the user, only for `needs_choice`. Every entry has `selected` null. */
            clarifications: components["schemas"]["Choice"][];
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
         *     - `rejected`: the guard refused the SQL: the generated one for a reason a fix cannot change (for example it is not
         *       a read-only SELECT, or it calls a function that is not allowed), or the one fixed in the `repair` round. Set:
         *       `message` (the reason), `query_id`, `sql_generated`; the `guard` step that stopped it has `ok` false.
         *     - `failed`: the SQL could not run and the one fix round did not help, either because the fixed SQL failed too or
         *       because the model failed during the fix; or the SQL ran past the time or memory limit, which gets no fix round.
         *       Set: `message`, `query_id`, `sql_generated`; `sql_guarded` unless that SQL never passed the guard. The `run` or
         *       `repair` step that stopped it has `ok` false.
         *
         *     A fix round starts when the analytics engine fails on the SQL, or when the guard refuses it because a table or
         *     column does not exist or because it could build a value too large (too deep, too many concatenated pieces, too
         *     wide, a size argument over the limit); there is at most one round per question. The status names the step that stopped the
         *     question: the guard gives `rejected`, the engine or the fix round gives `failed`. A model error before any SQL
         *     exists is not a status: the request gets 502 or 503.
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
        /** @description One cell: a JSON number, string or boolean; read it with the `kind` of its column. */
        CellValue: number | string | boolean;
        ChangePasswordRequest: {
            current_password: string;
            /** @description 8 to 128 characters. */
            new_password: string;
        };
        /**
         * @description `bar` for one dimension and one measure, `line` when the x axis is time.
         * @enum {string}
         */
        ChartKind: "bar" | "line";
        /** @description Suggested chart; null in `AskResponse` when the result is better shown as a table only. */
        ChartSuggestion: {
            kind: components["schemas"]["ChartKind"];
            /** @description Column on the x axis: a dimension for `bar`, time for `line`. A name from `result.columns`. */
            x: string;
            /** @description Columns drawn as series, names from `result.columns`. */
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
            /** Format: uuid */
            current_version_id: string | null;
            domain: string | null;
            /** Format: uuid */
            id: string;
            /** @description Table name used in SQL, derived from the dataset name. */
            logical_table: string;
            name: string;
            /**
             * @description Suggested questions of the current version, written by the semantic profile (#23); empty while that step has
             *     not finished.
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
            /** @description SQL that computed the groups of both slices, after the guard. */
            sql: string;
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
            /** @description One breakdown per dimension, the dimension that explains the change best first. */
            dimensions: components["schemas"]["DimensionBreakdown"][];
            /** @description Measure that changed, text for the end user (Vietnamese). */
            measure: string;
            /** @description Plain-language summary for the end user (Vietnamese). Every number in it appears in this breakdown. */
            summary: string;
            /**
             * @description False when the model's text failed the number check, or the model was not reached, and `summary` is a template
             *     sentence written by the backend.
             */
            summary_by_model: boolean;
            volume_price: components["schemas"]["VolumePrice"] | null;
        };
        /** @description One of the two slices. */
        ExplanationSlice: {
            /** @description SQL condition that selects the rows of the slice, on logical names. */
            filter: string;
            /** @description Text for the end user (Vietnamese) naming the slice, for example a quarter. */
            label: string;
            /** @description Value of the measure on the slice. */
            value: string;
        };
        /** @description One group of a dimension. `value` is null for the group of rows where the dimension column is null. */
        GroupContribution: {
            base_value: string;
            compare_value: string;
            /** @description `compare_value - base_value` of this group. */
            contribution: string;
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
         * @description Step of the backend:
         *     - `context`: read the dataset profile and build the prompt.
         *     - `llm`: the language model writes SQL.
         *     - `guard`: check the SQL, add row-level filters and a row limit.
         *     - `run`: run the SQL on the analytics engine.
         *     - `repair`: the language model fixes SQL that failed to run, or that the guard refused because a table or column
         *       does not exist or because it could build a value too large (at most once).
         *     - `explain`: compute the breakdown of a "why" question (code, no model).
         *     - `narrate`: the language model writes the summary of the breakdown.
         *     - `record`: save the question in the history.
         * @enum {string}
         */
        StepName: "context" | "llm" | "guard" | "run" | "repair" | "explain" | "narrate" | "record";
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
         * @enum {string}
         */
        WarningCode: "empty_result" | "all_null" | "filter_value_not_found" | "unusual_magnitude" | "explanation_unavailable" | "choice_not_applied";
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
            /** @description Not implemented yet */
            501: {
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
            /** @description 400 empty question or invalid JSON body, 401, 404 not a member or unknown dataset */
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
        requestBody?: never;
        responses: {
            /** @description Internal error; details are in the server log, not in the response */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Not implemented yet */
            501: {
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
}
