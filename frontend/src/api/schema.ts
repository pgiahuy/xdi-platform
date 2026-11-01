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
    "/workspaces/{ws}/catalog": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Datasets of the workspace with their tables
         * @description A dataset is one connected database or one uploaded file. Oldest first. Each dataset lists its tables (with columns of the current version) and the relations between them.
         */
        get: operations["listCatalog"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/catalog/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** One dataset with its tables */
        get: operations["getCatalogDataset"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/catalog/{id}/ask": {
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
         * @description Same response as `ask`. Without `table_id` the backend picks the table to start from: the one whose name, column names or column aliases match most words of the question, then the table of the conversation being continued, then the table holding most foreign keys.
         */
        post: operations["askCatalogDataset"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/catalog/{id}/conversations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Conversations of the caller on any table of a dataset
         * @description Newest first. Conversations with no recorded question are left out. Open one with `GET .../datasets/{table_id}/conversations/{id}`.
         */
        get: operations["listCatalogConversations"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/catalog/{id}/snapshot": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Snapshot every table of a database dataset again (admin only)
         * @description Copies every table this database dataset holds: tables snapshotted before and tables kept as metadata only, never tables the admin left unticked. One read-only REPEATABLE READ transaction, so all tables show the same moment; then every declared foreign key between held tables is recorded as a relation. A table whose content did not change gets no new version (`created: false`); the status is 200 when no table changed. No table count limit: a table over SOURCE_MAX_ROWS, or that would take the snapshot over SOURCE_SNAPSHOT_MAX_MB (Postgres sizes, smallest tables first), keeps metadata only (`metadata_only`); MAX_UPLOAD_MB applies to each copied table. A table the account can no longer read is skipped and listed in `unreadable`. SOURCE_RESNAPSHOT_HOURS runs the same on a schedule.
         */
        post: operations["snapshotCatalogDataset"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/data-roles": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Data roles of the workspace with their row rules and members (admin only)
         * @description A member who holds data roles sees only the rows their roles allow. Within one role, rules apply together (AND); between roles a row is visible when one role allows it (OR), counting only roles with a rule on the table read or on a table it points to. Admins are never filtered.
         */
        get: operations["listDataRoles"];
        put?: never;
        /** Create a data role without rules (admin only) */
        post: operations["createDataRole"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/data-roles/mine": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Data roles of the signed-in member, without their rules */
        get: operations["listMyDataRoles"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/data-roles/{role_id}": {
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
         * Delete a data role and its rules (admin only)
         * @description Refused while a member holds the role: a member left without roles would see every row.
         */
        delete: operations["deleteDataRole"];
        options?: never;
        head?: never;
        /** Rename a data role or change its description (admin only) */
        patch: operations["updateDataRole"];
        trace?: never;
    };
    "/workspaces/{ws}/data-roles/{role_id}/policies": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        /**
         * Replace the row rules of a data role (admin only)
         * @description Each rule names a table, a dimension column of its current version and the values the role sees. Values missing from the column profile are saved with a warning. Takes effect from the next request of each member.
         */
        put: operations["setDataRolePolicies"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/datasets": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Tables of the workspace, every dataset mixed (deprecated: use listCatalog)
         * @deprecated
         * @description Each item is one table; `parent_id` is its dataset in `GET .../catalog`. Kept for the current UI.
         */
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
        /** Delete a dataset with all its versions, files and question history (admin only) */
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
         * Ask a question starting from one table (deprecated: use askCatalogDataset with table_id)
         * @deprecated
         * @description The path id is a table id (`DatasetTable.id`). One JSON response, no streaming. A question that cannot be answered (`rejected`, `needs_choice`, `failed`) is still 200, with `status` and `message` in the body and the evidence gathered so far. Error codes are for invalid requests, access, and the language model service.
         */
        post: operations["ask"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/datasets/{id}/columns/aggregation": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        /**
         * Set the default aggregation of a measure column (admin only)
         * @description Applies to the current version. The SQL prompt asks for AVG on `avg` columns, and why questions on them compare the average per row.
         */
        put: operations["setColumnAggregation"];
        post?: never;
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
         * Conversations of the caller on one table (deprecated: use listCatalogConversations)
         * @deprecated
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
         * @description Turns oldest first, each with its question, status, SQL, the short text of what was understood and the answer text shown when it was asked. Result rows are not stored: ask a turn again to see its table and chart.
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
    "/workspaces/{ws}/datasets/{id}/examples/export": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Approved example questions of a dataset as candidate evaluation cases (admin only)
         * @description A JSON array in the format of the case files in `eval/` (`id`, `question`, `expected_sql`, `ordered`). The SQL is the one the model wrote; review every case before adding it to a case file.
         */
        get: operations["exportExamples"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/datasets/{id}/filter-columns": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Columns of a table a row rule can use, with their known values (admin only) */
        get: operations["listFilterColumns"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/datasets/{id}/metrics": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Metrics of a dataset and the columns of its current version */
        get: operations["listMetrics"];
        put?: never;
        /**
         * Create a metric (admin only)
         * @description The expression is run once on the current version, through the SQL guard, before it is saved. The metric is confirmed and used by the next question.
         */
        post: operations["createMetric"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/datasets/{id}/metrics/{metric_id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        /**
         * Save a new version of a metric (admin only)
         * @description Adds a version with the next number and keeps the earlier ones. The name cannot change. Checked like a new metric.
         */
        put: operations["updateMetric"];
        post?: never;
        /**
         * Delete a metric with all its versions (admin only)
         * @description Earlier questions keep the SQL they ran.
         */
        delete: operations["deleteMetric"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/datasets/{id}/overview": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * What a dataset holds: summary, column profile, metrics and pre-built charts
         * @description For every member who can ask the dataset. No language model is called. Charts that read the data go through the SQL guard with the viewer's row filters and are kept per version; row counts by value come from the column profile. Sensitive columns return no values.
         */
        get: operations["getDatasetOverview"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/domains": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Industry packs of the workspace with all their concepts
         * @description The first call in a workspace copies the packs of `data/domains/` into it; later calls add only packs and concepts it does not have yet and never overwrite an edit.
         */
        get: operations["listConcepts"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/domains/{pack}/concepts": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Add a concept to a pack (admin only)
         * @description The concept goes last in the pack and is used by the next question on a dataset with this pack.
         */
        post: operations["createConcept"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/domains/{pack}/concepts/{concept_id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        /**
         * Replace a concept (admin only)
         * @description Checked like a new concept. Send `enabled: false` to leave the concept out of the prompt.
         */
        put: operations["updateConcept"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/error-log": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Where the model went wrong, newest first (admin only)
         * @description Written automatically: guard refusals, SQL that failed to run, blocked double counting over a join, invalid why forms, text rejected by the number or direction check, and answers rated `bad`. At most 200 rows.
         */
        get: operations["listErrorLog"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/examples": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Answers rated good, to review as example questions (admin only)
         * @description Answered questions of every member of the workspace rated `good`, newest rating first, with whether an admin approved them.
         */
        get: operations["listExamples"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/examples/{query_id}/approval": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        /**
         * Approve an answer rated good as an example question (admin only)
         * @description Its question and the SQL the model wrote (for a why question, the form) become examples in the prompt of later questions on the same dataset that share a word with it. Records who approved and when.
         */
        put: operations["approveExample"];
        post?: never;
        /** Withdraw the approval of an example question (admin only) */
        delete: operations["withdrawExample"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/lessons": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Lessons of the workspace, newest first (admin only) */
        get: operations["listLessons"];
        put?: never;
        /**
         * Write a lesson as a draft (admin only)
         * @description A draft is not used. Enable it to approve it; then it goes into the prompts of related questions.
         */
        post: operations["createLesson"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/lessons/{lesson_id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        /** Delete a lesson (admin only) */
        delete: operations["deleteLesson"];
        options?: never;
        head?: never;
        /**
         * Enable (approve) or disable a lesson (admin only)
         * @description Enabling records who approved it and when. At most 3 enabled lessons related to a question go into its prompts: same dataset or the whole workspace, sharing a word with the question or naming a column it matched.
         */
        patch: operations["setLessonState"];
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
    "/workspaces/{ws}/members/{user_id}/data-roles": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        /**
         * Replace the data roles of a member (admin only)
         * @description An empty list removes every role: the member then sees every row. Only roles with at least one rule can be given.
         */
        put: operations["setMemberDataRoles"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/queries/{query_id}/feedback": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        /**
         * Rate an answer of the caller
         * @description Replaces the earlier rating. Changing a `good` rating to `bad` also withdraws its approval as an example question.
         */
        put: operations["rateAnswer"];
        post?: never;
        /**
         * Remove the rating of an answer of the caller
         * @description Also withdraws its approval as an example question.
         */
        delete: operations["clearRating"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/reports": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Reports of the caller
         * @description Last changed first, without items.
         */
        get: operations["listReports"];
        put?: never;
        /**
         * Create an empty report
         * @description Add answers with addReportItem. Every member can create reports; only the creator reads them.
         */
        post: operations["createReport"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/reports/automatic": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Build an automatic report for one dataset
         * @description The backend builds a fixed set of analyses with its own SQL (no SQL from the language model): the main measure over time with the strongest change and its breakdown, then the measure by two dimensions with the top groups, concentration and groups far from the average. Every query goes through the guard with the caller's row filters. The language model only rewords the computed facts, with the same two checks as an answer; `report.model_calls` counts its calls. One JSON response, no streaming.
         */
        post: operations["createAutomaticReport"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/reports/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Open a report with every item rendered again
         * @description Result rows are not stored: the stored SQL of every item runs again through the guard with the caller's access, on the data versions the question read. An item whose SQL cannot run again has `error` and is still listed.
         */
        get: operations["getReport"];
        put?: never;
        post?: never;
        /**
         * Delete a report
         * @description The questions of its items stay in the history.
         */
        delete: operations["deleteReport"];
        options?: never;
        head?: never;
        /** Change the title or the intro of a report */
        patch: operations["updateReport"];
        trace?: never;
    };
    "/workspaces/{ws}/reports/{id}/items": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** Add an answered question of the caller at the end of a report */
        post: operations["addReportItem"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/reports/{id}/items/{item_id}": {
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
         * Remove an item from a report
         * @description The question stays in the history.
         */
        delete: operations["removeReportItem"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/reports/{id}/order": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        /** Set the order of the items */
        put: operations["reorderReportItems"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/workspaces/{ws}/reports/{id}/xlsx": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Download a report as an Excel file
         * @description Built from the same rendering as getReport. Sheets: a summary (one row per item with its answer and checks), one sheet per item with its result table and breakdown, and `Cách tính` (for every item: measure, filters, grouping, period, SQL that ran, tables and versions, time asked).
         */
        get: operations["exportReportXlsx"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
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
         * Tables linked to a table by foreign keys, in both directions (admin only)
         * @description Read from pg_catalog, one hop around the table. `many_to_one` lists the tables its foreign keys point to, `one_to_many` the tables whose foreign keys point to it. A table is addable to the snapshot when the account can read it and the referenced columns are the primary key or a UNIQUE constraint; otherwise `reason` says why.
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
         * Snapshot tables of the source into its database dataset, every readable table by default (admin only)
         * @description Without `table`, `tables` and `dataset_id`, every table and view the account can read is snapshotted; `tables` lists the ticked tables instead, and one name snapshots that table alone (for example one kept as metadata only). All tables are copied in one read-only REPEATABLE READ transaction, then stored as Parquet versions like uploaded files; a table snapshotted before from this source gets a new version of that table, and one with unchanged content creates nothing (`created` false). Every declared foreign key between tables snapshotted from this source is recorded as a relation; relations are never inferred. Limits are checked on Postgres sizes before copying: a table over SOURCE_TABLE_MAX_MB, or that would take the snapshot over SOURCE_SNAPSHOT_MAX_MB (smallest tables first), or over SOURCE_MAX_ROWS when that is set, keeps columns, types, keys and comments only (`metadata_only`). There is no row limit by default. `dataset_id` (a table id) snapshots that table again with every table linked to it.
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
        /**
         * Tables and views the source account can read, for picking what to snapshot (admin only)
         * @description Every table and view the account can read (its SELECT grants decide the scope), with the row estimate and size from Postgres statistics, the table comment, foreign keys in and out among readable tables, `is_referenced` when another table points to it (warn before unticking a lookup table), `over_limit` when the table alone is over SOURCE_TABLE_MAX_MB (or SOURCE_MAX_ROWS when set), and what this source already holds for it (`state`).
         */
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
        AddReportItemRequest: {
            /**
             * Format: uuid
             * @description `AskResponse.query_id` (or `TurnItem.query_id`) of an answered question the caller asked.
             */
            query_id: string;
        };
        /**
         * @description Verdict of the two checks on a text written by the language model: `match` (agrees with the computed numbers),
         *     `warning` (some parts can be read several ways, not certainly wrong), `mismatch` (wrong on every attempt; the model's
         *     text is replaced by a template sentence).
         * @enum {string}
         */
        AnswerVerdict: "match" | "warning" | "mismatch";
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
             *     `insight.summary` when the language model wrote it and it passed the checks (the model only sees the sentences
             *     the backend computed, never result rows), otherwise a template sentence written by the backend. A list of rows
             *     without aggregates always gets the template (row count and filters). Null otherwise; a "why" question has
             *     `explanation.summary`.
             */
            answer_text: string | null;
            /**
             * @description True when `answer_text` is the language model's text of `insight.summary`, false for the template sentence (no
             *     checked model text, a list of rows, or `ANSWER_TEXT=false`). Null when `answer_text` is null.
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
            data_range: components["schemas"]["DataRange"] | null;
            explanation: components["schemas"]["Explanation"] | null;
            insight: components["schemas"]["Insight"] | null;
            /**
             * @description Text for the end user (Vietnamese): the reason for `rejected` and `failed`, the prompt for `needs_choice`;
             *     null for `answered`.
             */
            message: string | null;
            /** @description Language model configured for the question, for example `gemini-2.5-flash` or `mock`. */
            model: string;
            /**
             * @description Every call to the language model for this question, in the order the calls finished, with wall time and tokens.
             *     The sum of `duration_ms` is the time spent waiting for the model; the rest of the step time is the backend.
             */
            model_calls: components["schemas"]["LlmCall"][];
            /**
             * Format: uuid
             * @description Id of the question in the history (`queries`); null for `needs_choice`.
             */
            query_id: string | null;
            /** @description The question exactly as sent in `AskRequest.question`. */
            question: string;
            question_type: components["schemas"]["QuestionType"] | null;
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
            /**
             * Format: uuid
             * @description Table the question started from (the table routes `/datasets/{id}/drill` and `/conversations` take this id).
             *     Asked on a whole dataset (`/catalog/{id}/ask`), the backend picks it; asked on a table, it is that table.
             */
            table_id: string;
            /**
             * @description Tables `sql_guarded` read, in the order the query first reads them: the dataset asked about, and related
             *     datasets of the same database snapshot when the SQL joins them. Empty unless the status is `answered`.
             */
            tables_read: components["schemas"]["TableRead"][];
            understanding: components["schemas"]["Understanding"] | null;
            /**
             * Format: uuid
             * @description Version of `table_id` the question ran on.
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
        AutoReportRequest: {
            /**
             * Format: uuid
             * @description Dataset to analyse (`DatasetSummary.id`).
             */
            dataset_id: string;
        };
        /** @description The automatic report just built, and the sections that could not be built. */
        AutoReportResponse: {
            report: components["schemas"]["ReportSummary"];
            /** @description One Vietnamese sentence per planned section left out, with the reason. */
            skipped: string[];
        };
        /** @description One group of a breakdown table in one slice. */
        BreakdownKey: {
            /** @description `DimensionBreakdown.column`: a column name of the dataset, not of `result.columns`. */
            column: string;
            slice: components["schemas"]["SliceName"];
            value: components["schemas"]["CellValue"] | null;
        };
        /** @description A common way to compute the concept, in words, not tied to the columns of a dataset. */
        Calculation: {
            /** @description What it computes and when to pick it over the others. */
            description: string;
            /** @description Short name of the method. */
            name: string;
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
         * @description How the values of a measure column are combined when no metric says otherwise: `sum` for amounts and quantities,
         *     `avg` for values that describe one row (age, score, credits, days).
         * @enum {string}
         */
        ColumnAggregation: "sum" | "avg";
        /** @description New default aggregation of one measure column of the current version. */
        ColumnAggregationRequest: {
            aggregation: components["schemas"]["ColumnAggregation"];
            column: string;
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
        /** @description A business concept of an industry pack in this workspace. */
        Concept: {
            calculations: components["schemas"]["Calculation"][];
            description: string;
            dimensions: string[];
            /** @description False for a concept an admin turned off; it stays here but is left out of the prompt. */
            enabled: boolean;
            /** Format: uuid */
            id: string;
            /** @description Id of the concept in the pack file (`data/domains/<pack>.json`); null for a concept an admin added. */
            key: string | null;
            name: string;
            sanity_checks: components["schemas"]["SanityCheck"][];
            /** @description Other words for the concept, Vietnamese and English; the prompt lists them next to the name. */
            synonyms: string[];
            /**
             * Format: date-time
             * @description Last edit on the concepts screen; null when unchanged since it was copied from the pack file.
             */
            updated_at: string | null;
        };
        /** @description An industry pack of the workspace with all its concepts, enabled or not. */
        ConceptPack: {
            /** @description In prompt order: the order of the pack file, concepts added by admins last. */
            concepts: components["schemas"]["Concept"][];
            description: string;
            /** @description Pack id, the file name without `.json`; a dataset uses the pack whose key is in its `domain`. */
            key: string;
            name: string;
        };
        /** @description A concept to add, or the new content of one. Errors name the wrong field in `field`. */
        ConceptRequest: {
            /** @description At most three. */
            calculations?: components["schemas"]["Calculation"][];
            description: string;
            dimensions?: string[];
            /** @description Defaults to true. */
            enabled?: boolean;
            /** @description Unique in the pack, compared ignoring case and where the tone mark sits (not ignoring tone marks). */
            name: string;
            sanity_checks?: components["schemas"]["SanityCheck"][];
            /** @description At least one; a word may belong to only one concept of the pack, compared without case and accents. */
            synonyms: string[];
        };
        /** @description Industry packs of the workspace, sorted by key. */
        ConceptsResponse: {
            packs: components["schemas"]["ConceptPack"][];
        };
        /** @description One conversation on a dataset. */
        ConversationItem: {
            /** Format: date-time */
            created_at: string;
            /** Format: uuid */
            id: string;
            /**
             * Format: uuid
             * @description Table the conversation is stored on: open it with `GET .../datasets/{table_id}/conversations/{id}`.
             */
            table_id: string;
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
        CreateReportRequest: {
            intro?: string;
            title: string;
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
            /** @description PEM CA certificate(s) trusted for `verify-full`, besides the system CAs. Only with `verify-full`. */
            ssl_root_cert?: string | null;
            /**
             * @description `disable`, `prefer` (default), `require` (TLS, certificate not checked) or `verify-full` (TLS, certificate
             *     signed by a trusted CA and matching the host name).
             */
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
        /** @description Dates the data covers, see `AskResponse.data_range`. */
        DataRange: {
            /** @description The time column, as named in the dataset. */
            column: string;
            /** @description First and last day with data, `YYYY-MM-DD`. */
            from: string;
            /** @description Vietnamese name of the column from its profile; null when the profile has none. */
            label: string | null;
            /**
             * @description True when the SQL that ran filters on a date after the last day with data: the period asked about is not fully
             *     covered (or not covered at all).
             */
            partial: boolean;
            to: string;
        };
        /** @description Create or rename a data role. */
        DataRoleRequest: {
            /** @description Optional, at most 500 characters. */
            description?: string | null;
            /** @description 1 to 60 characters, unique in the workspace (case-insensitive), for example `Sales South`. */
            name: string;
        };
        /** @description A data role with its row rules and the members who hold it. */
        DataRoleResponse: {
            description: string | null;
            /** Format: uuid */
            id: string;
            /** @description Account ids of the members who hold the role. */
            member_ids: string[];
            name: string;
            /** @description Rules of the role, by table and column. Rules on one table all apply together (AND). */
            policies: components["schemas"]["RolePolicy"][];
        };
        /** @description A data role of the workspace. */
        DataRoleRow: {
            description?: string | null;
            /** Format: uuid */
            id: string;
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
            /**
             * Format: date-time
             * @description When the current version was created: upload time, or the last snapshot of a database source.
             */
            current_version_at: string | null;
            /** Format: uuid */
            current_version_id: string | null;
            /**
             * @description Table comment written in the source database (`COMMENT ON TABLE`), from the current version; null for files
             *     and tables without a comment.
             */
            description: string | null;
            domain: string | null;
            /** Format: uuid */
            id: string;
            /** @description Table name used in SQL, derived from the dataset name. */
            logical_table: string;
            name: string;
            /**
             * Format: uuid
             * @description Dataset (one database or one file, `GET .../catalog/{id}`) this table belongs to. The table routes under
             *     `/datasets/{id}` take the table id; a file has one table.
             */
            parent_id: string;
            /**
             * @description Datasets linked to this one by a foreign key of the source database (snapshots of several related tables);
             *     empty for uploaded files.
             */
            relations: components["schemas"]["RelatedDataset"][];
            /**
             * Format: int64
             * @description Row count of the current version; null while the table has no current version.
             */
            row_count: number | null;
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
        /** @description A question on a whole dataset, or starting from one of its tables. */
        DatasetAskRequest: {
            /** @description Same as `AskRequest.choices`. */
            choices?: components["schemas"]["ChosenOption"][];
            /**
             * Format: uuid
             * @description Conversation to continue, from `AskResponse.conversation_id`. A conversation on any table of this dataset can
             *     be continued; one of another user or another dataset is 404.
             */
            conversation_id?: string | null;
            question: string;
            /**
             * Format: uuid
             * @description Table to start from (`DatasetTable.id`). Leave out to let the backend pick it from the question; the answer
             *     tells which one in `AskResponse.table_id`. Either way the question also reads tables linked to it by relations
             *     (two steps, eight tables at most).
             */
            table_id?: string | null;
        };
        /**
         * @description What a dataset is.
         * @enum {string}
         */
        DatasetKind: "database" | "file";
        /** @description Overview of a dataset for anyone who can ask it: what it holds and what to ask, computed without the language model. */
        DatasetOverviewResponse: {
            /** @description Pre-built charts: the main measure over time, row count over time, then row count by value of each dimension. */
            charts: components["schemas"]["OverviewChart"][];
            /** @description Columns in table order, without the ones the profile ignores (all empty, `_source_file`). */
            columns: components["schemas"]["OverviewColumn"][];
            /** Format: uuid */
            dataset_id: string;
            /**
             * @description Description of the table written in the source database (`COMMENT ON TABLE`) for the current version; null for
             *     uploaded files and tables without a comment.
             */
            description: string | null;
            main_measure: components["schemas"]["MainMeasure"] | null;
            /** @description Defined metrics, one per name: the latest confirmed definition, else the latest draft. */
            metrics: components["schemas"]["OverviewMetric"][];
            /** @description Parts that are not there yet or cannot be shown, with the reason. */
            missing: components["schemas"]["MissingPart"][];
            name: string;
            summary: components["schemas"]["OverviewSummary"];
            version: components["schemas"]["OverviewVersion"] | null;
        };
        /** @description A foreign-key relation between two datasets of the workspace, copied from the source database. */
        DatasetRelation: {
            /** @description `many_to_one`, or `one_to_one` when the foreign-key columns are also unique. */
            cardinality: string;
            /** @description Foreign-key constraint name in the source database. */
            constraint_name: string;
            from_columns: string[];
            /**
             * Format: uuid
             * @description Dataset holding the foreign key (the many side).
             */
            from_dataset_id: string;
            from_dataset_name: string;
            to_columns: string[];
            /**
             * Format: uuid
             * @description Dataset whose primary key or UNIQUE columns the foreign key points to (the one side).
             */
            to_dataset_id: string;
            to_dataset_name: string;
        };
        /** @description A dataset: one connected database or one uploaded file, with its tables. */
        DatasetSummary: {
            /** Format: date-time */
            created_at: string;
            /** @description Domain pack bound to the dataset (`key` of `GET .../domains`); null when none. */
            domain: string | null;
            /**
             * Format: uuid
             * @description Dataset id. For a database, the same id as its source in `/sources/{id}` (admin only).
             */
            id: string;
            kind: components["schemas"]["DatasetKind"];
            /**
             * @description Tables of the database kept with columns, types, keys and comments but no rows (over SOURCE_MAX_ROWS or the
             *     SOURCE_SNAPSHOT_MAX_MB total), ordered by `table`. An admin snapshots one with `POST /sources/{id}/snapshot`
             *     and `tables: [table]`. Empty for a file.
             */
            metadata_only: components["schemas"]["MetadataOnlyTable"][];
            /** @description Database name for a database, the name given at upload for a file. */
            name: string;
            /** @description Foreign-key relations between tables of this dataset, copied from the source database. Empty for a file. */
            relations: components["schemas"]["DatasetRelation"][];
            /**
             * @description Questions picked by hand for this dataset (the sample databases set in SAMPLE_DATABASE_URL); empty otherwise.
             *     The ask page shows them instead of the tables' `suggested_questions` while no table is pinned.
             */
            suggested_questions: string[];
            /**
             * @description Tables, ordered by `source_table` (schema, then table) and then by name. A file has one table; a database
             *     connected but not snapshotted yet has none.
             */
            tables: components["schemas"]["DatasetTable"][];
            /**
             * Format: date-time
             * @description Latest `current_version_at` of its tables: the last snapshot or upload. Null while no table has data.
             */
            updated_at: string | null;
        };
        /**
         * @description A table of a dataset. Its id is the id the table routes take: `/datasets/{id}/overview`, `/metrics`, `/drill`,
         *     `/conversations/{conversation_id}`.
         */
        DatasetTable: {
            /** @description Columns of the current version, in table order; empty without a current version. */
            columns: components["schemas"]["TableColumn"][];
            /**
             * @description `pending`, `structure`, `semantic` or `failed`; null without a current version. Asking works from
             *     `structure` on.
             */
            current_profile_status: string | null;
            /**
             * Format: date-time
             * @description When the current version was created: upload time, or the last snapshot.
             */
            current_version_at: string | null;
            /** Format: uuid */
            current_version_id: string | null;
            /** @description Table comment from the source database (`COMMENT ON TABLE`) at the current version; null when none. */
            description: string | null;
            /** Format: uuid */
            id: string;
            /** @description Table name used in SQL; unique within the dataset only, so two databases can both have `orders`. */
            logical_table: string;
            name: string;
            /**
             * Format: int64
             * @description Rows in the current version; null while the table has no data.
             */
            row_count: number | null;
            /** @description Schema in the source database (`sales` for `sales.orders`); null for a file. */
            schema: string | null;
            /** @description Same meaning as `Dataset.semantic_unavailable`. */
            semantic_unavailable: boolean;
            /** @description `schema.table` in the source database; null for a file. */
            source_table: string | null;
            /** @description Suggested questions of the current version; empty until the semantic profile is done. */
            suggested_questions: string[];
        };
        DatasetVersion: {
            /** Format: date-time */
            created_at: string;
            /** Format: uuid */
            dataset_id: string;
            /**
             * @description Description of the snapshotted table written in the source database (`COMMENT ON TABLE`); null for uploaded
             *     files and tables without a comment.
             */
            description: string | null;
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
            /**
             * @description True when at least one group is an `outlier`. Dimensions with outliers come first, each part in the order of
             *     how well the dimension explains the change.
             */
            has_outliers: boolean;
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
        /** @description One measure of the clicked number, computed again over its source rows. */
        DrillMeasure: {
            /**
             * @description Name of the measure column in the answer's `result.columns` (for a breakdown row, the column of the clicked
             *     slice in the breakdown query).
             */
            column: string;
            /**
             * @description With rule `sum`: the column of `result` that holds each row's part, so the rows of all pages add up to
             *     `value`. null otherwise.
             */
            row_column?: string | null;
            rule: components["schemas"]["MeasureRule"];
            value: components["schemas"]["CellValue"] | null;
        };
        /**
         * @description Which number to trace back to its source rows: set exactly one of `cell` and `breakdown`. Rows come one page at a
         *     time, sorted by every column of the page, so pages do not overlap.
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
             * @description Columns of `result` that come from the joined tables rather than from `source_table`: the grouping keys and
             *     filter columns of the question on those tables. Empty when the question reads one table.
             */
            joined_columns: string[];
            /**
             * @description The measures of the clicked number, each computed again over all source rows (every page). For a breakdown row,
             *     only the measure of the clicked slice.
             */
            measures: components["schemas"]["DrillMeasure"][];
            /**
             * @description The page. For a question on one table: every column of the dataset, including `_source_file`, the file each
             *     row came from. For a question that joins tables: every column of `source_table`, then `joined_columns`. Then,
             *     for each `sum` measure over an expression rather than one column of the page, its `row_column`. Here `result.total_rows` counts the rows of this page only.
             */
            result: components["schemas"]["ResultTable"];
            /**
             * @description The data table the rows come from. For a question that joins tables, this is the table the measures add up
             *     (the many side, such as `order_items` joined to `orders` and `customers`): one row of `result` per row of it
             *     that went into the number, its columns first. null when the rows come from a WITH table or a subquery that
             *     does not start from a data table.
             */
            source_table?: string | null;
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
        /**
         * @description - `separates_slices`: every value of the column is found in one slice only, so the column only renames the two
         *       slices.
         *     - `duplicates`: the column splits the rows into exactly the groups of another column (`duplicate_of`).
         * @enum {string}
         */
        DropReason: "separates_slices" | "duplicates";
        /** @description One dimension left out, and why. */
        DroppedDimension: {
            /** @description For `separates_slices`: values found only in the base slice, as text. */
            base_only: string[];
            column: string;
            /** @description For `separates_slices`: values found only in the compared slice, as text. */
            compare_only: string[];
            /** @description Column with the same groups, for `duplicates`. */
            duplicate_of: string | null;
            /** @description Vietnamese name of the column from the column profile; null when the profile has none. */
            label: string | null;
            reason: components["schemas"]["DropReason"];
        };
        /** @description Error body returned by every endpoint on failure. */
        ErrorBody: {
            code: components["schemas"]["ErrorCode"];
            /** @description Name of the request field the error is about, when it is about one field. Omitted otherwise. */
            field?: string | null;
            /** @description Human-readable explanation in Vietnamese, meant to be shown to the end user as is. */
            message: string;
        };
        /**
         * @description Stable machine-readable error code; one per HTTP status.
         * @enum {string}
         */
        ErrorCode: "bad_request" | "unauthorized" | "forbidden" | "not_found" | "too_large" | "internal" | "not_implemented" | "upstream" | "unavailable";
        /** @description One row of the error log. */
        ErrorEntry: {
            /** Format: date-time */
            created_at: string;
            /** Format: uuid */
            dataset_id: string | null;
            dataset_name: string | null;
            error: string;
            /** Format: uuid */
            id: string;
            /** @description What the model returned at that step: SQL, the form as JSON, or the text it wrote. */
            model_output: string | null;
            /**
             * Format: uuid
             * @description The recorded question; null when it was not recorded (waiting for a choice) or was deleted.
             */
            query_id: string | null;
            question: string;
            step: components["schemas"]["ErrorStep"];
        };
        /** @description Newest rows first. */
        ErrorLogResponse: {
            entries: components["schemas"]["ErrorEntry"][];
        };
        /**
         * @description The step where the model went wrong.
         * @enum {string}
         */
        ErrorStep: "guard" | "run" | "fanout" | "why_form" | "narrate" | "feedback";
        /**
         * @description One approved example as a candidate case for the evaluation set, in the format of the files in `eval/` (for
         *     example `eval/dataco.json`). Review each case before adding it: check the SQL and set `ordered`.
         */
        EvalCaseCandidate: {
            expected_sql: string[];
            id: string;
            /** @description Always false here; set it to true when the row order is part of the answer (top N, time series). */
            ordered: boolean;
            question: string;
        };
        /** @description An answer rated `good`, for admins to review as an example question. */
        ExampleCandidate: {
            /**
             * Format: date-time
             * @description Null until an admin approves it as an example question.
             */
            approved_at: string | null;
            approved_by: string | null;
            /**
             * Format: uuid
             * @description Null when the dataset was deleted.
             */
            dataset_id: string | null;
            dataset_name: string | null;
            /** Format: uuid */
            query_id: string;
            question: string;
            /** Format: date-time */
            rated_at: string;
            /** @description Name or email of who rated it; null when the account was deleted. */
            rated_by: string | null;
            /**
             * Format: int64
             * @description Rows of the result when it was asked; results are not stored.
             */
            row_count: number | null;
            /** @description SQL the model wrote, before the guard; for a why question, the SQL built from its form. */
            sql: string | null;
            /** @description Short text of the "understood as" card; null when the question was not asked in a conversation. */
            understood: string | null;
            /** @description A why question: its example in the prompt is the form the model filled, not SQL. */
            why: boolean;
        };
        /** @description Answers rated `good`, newest rating first. */
        ExampleCandidatesResponse: {
            examples: components["schemas"]["ExampleCandidate"][];
        };
        /**
         * @description Breakdown of a change between two slices (two periods or two groups); see `AskResponse.explanation` for when it is
         *     set. Numbers are exact strings of decimal digits.
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
            /**
             * @description Dimensions left out of `dimensions` or of the summary, with the reason; show them as a note with the
             *     breakdown so that a missing table is explained.
             */
            dropped_dimensions: components["schemas"]["DroppedDimension"][];
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
            /**
             * @description Plain-language summary for the end user (Vietnamese), as one text: `summary_title` then each point as
             *     "lead: text". When `summary_by_model` the model wrote it from the numbers the backend computed, its claims were
             *     checked against this breakdown and an independent model call compared the meaning of the text with the claims
             *     and the names of the two slices; otherwise the backend wrote it from a template.
             */
            summary: string;
            /**
             * @description False when `summary` is a template sentence written by the backend: the change is zero (the model is not
             *     called), the model was not reached, the independent check could not judge the text, or
             *     the text failed the checks twice (one rewrite is asked).
             */
            summary_by_model: boolean;
            /**
             * @description Points under the main answer (Vietnamese). When `summary_by_model`, the main causes the model wrote, all of kind
             *     `reason`; otherwise written by the backend, each said only when the numbers support it.
             */
            summary_points: components["schemas"]["SummaryPoint"][];
            /**
             * @description Main answer, one sentence (Vietnamese): written by the model when `summary_by_model`, otherwise by the backend
             *     from the numbers above.
             */
            summary_title: string;
            summary_verdict: components["schemas"]["AnswerVerdict"] | null;
            /**
             * @description Parts of the model's text that the independent check found unclear but not certainly wrong, one Vietnamese
             *     sentence each, written by the checking model; show them with `summary`. Empty when nothing was unclear or
             *     `summary` is a template sentence.
             */
            summary_warnings: string[];
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
        /**
         * @description Why an answer is `bad`.
         * @enum {string}
         */
        FeedbackReason: "misunderstood" | "wrong_numbers" | "unclear_explanation" | "other";
        /** @description Rating of one answer by the member who asked it. Sending a new rating replaces the old one. */
        FeedbackRequest: {
            /** @description Optional short note, only with `bad`, at most 500 characters. */
            note?: string | null;
            rating: components["schemas"]["Rating"];
            reason?: components["schemas"]["FeedbackReason"] | null;
        };
        /** @description A column a row rule can use: a dimension column of the table's current version. */
        FilterColumnResponse: {
            /** @description Vietnamese label: the first alias written by the semantic profile; null until then. */
            label: string | null;
            name: string;
            /**
             * @description Known values of the column from its profile, most rows first; empty for a sensitive column or one with too
             *     many values to list. A rule may still use other values.
             */
            values: string[];
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
             * @description True when the group stands out: its `share_of_change` is at least 1 (in absolute value) and its
             *     `percent_change` is at least 10 points away from the `percent_change` of the whole, or it is `new`, `gone` or a
             *     `sign_flip`.
             */
            outlier: boolean;
            /**
             * @description `(compare_value - base_value) / base_value * 100`, one decimal. Null when the group is `new` or `gone`, when
             *     `base_value` is zero or negative, or when `sign_flip`.
             */
            percent_change: string | null;
            presence: components["schemas"]["GroupPresence"];
            /**
             * @description Ratio measures only: part of `change` because the ratio inside this group changed,
             *     `(base_share + compare_share) / 2 * (compare_value - base_value)`.
             */
            rate_effect: string | null;
            /**
             * @description `contribution / change * 100`, one decimal: negative when the group moved against the change, above 100 when
             *     other groups offset it. Null when the change is close to zero (under 0.5% of the larger slice), where these
             *     percents grow without meaning.
             */
            share_of_change: string | null;
            /**
             * @description True when the base and compared values are both non-zero with opposite signs, for example a profit that
             *     turned into a loss.
             */
            sign_flip: boolean;
            value: components["schemas"]["CellValue"] | null;
        };
        /**
         * @description Where a group has rows: `both` slices, only the compared slice (`new`), or only the base slice (`gone`). For a
         *     ratio, a group has rows in a slice when its denominator there is not zero.
         * @enum {string}
         */
        GroupPresence: "both" | "new" | "gone";
        HealthResponse: {
            /** @description Always `ok` when the server answers. */
            status: string;
        };
        /** @description Explanation of a plain answer, chosen by the shape of the result. */
        Insight: {
            kind: components["schemas"]["InsightKind"];
            /**
             * @description False when the numbers show nothing notable: a change below the threshold, too few rows to conclude, groups of
             *     about the same size, or a plain list of rows.
             */
            notable: boolean;
            /** @description Points under the main finding, written by the backend (Vietnamese). */
            points: components["schemas"]["InsightPoint"][];
            /**
             * @description Plain-language text for the end user (Vietnamese): the language model's wording of `title` and `points` when it
             *     passed the checks, otherwise `title` and `points` joined.
             */
            summary: string;
            /**
             * @description True when `summary` was written by the language model; false when it is the backend's text (the model failed,
             *     timed out, its text failed the checks on every attempt, or `INSIGHT_TEXT=false`). The checks: every number and
             *     direction must come from the points, no causal word, then a separate model call compares the meaning of the text
             *     with `title` and `points`; when a check fails or finds an unclear point, the model rewrites once.
             */
            summary_by_model: boolean;
            summary_verdict: components["schemas"]["AnswerVerdict"] | null;
            /**
             * @description Parts of `summary` that the separate check found unclear but not certainly wrong, one Vietnamese sentence each,
             *     written by the checking model; show them with `summary`. Empty when nothing was unclear or `summary` is the
             *     backend's text.
             */
            summary_warnings: string[];
            /** @description Main finding, one sentence written by the backend (Vietnamese). */
            title: string;
        };
        /**
         * @description - `single_value`: one number; compared with the period before when the question names a period.
         *     - `time_series`: one measure over periods; overall direction and the period that changed most against the one
         *       before, broken down by dimension in `AskResponse.explanation`.
         *     - `groups`: one measure over the groups of one column; share of the top group, how many groups make 80% of the
         *       total, groups far from the mean.
         *     - `top_n`: the first groups by a measure (the SQL has a limit); their share of the total and the gap between the
         *       first two.
         *     - `rows`: anything else, such as rows without aggregates; only the number of rows and the filters.
         * @enum {string}
         */
        InsightKind: "single_value" | "time_series" | "groups" | "top_n" | "rows";
        /** @description One point of an insight: a short lead (shown in bold) and the sentence after it. */
        InsightPoint: {
            /** @description Lead of the point (Vietnamese), for example the words for "concentration". */
            lead: string;
            /** @description Rest of the point (Vietnamese). */
            text: string;
        };
        /** @description A short lesson for the model, written by an admin. */
        Lesson: {
            /** Format: date-time */
            approved_at: string | null;
            /** @description Who last enabled it; null for a draft never enabled. */
            approved_by: string | null;
            /** Format: date-time */
            created_at: string;
            created_by: string | null;
            /**
             * Format: uuid
             * @description Null for a lesson of the whole workspace.
             */
            dataset_id: string | null;
            dataset_name: string | null;
            /** @description Approved and in use: only these lessons go into prompts. */
            enabled: boolean;
            /**
             * Format: uuid
             * @description The error log row it was written from.
             */
            error_id: string | null;
            /** Format: uuid */
            id: string;
            text: string;
        };
        /** @description A new lesson, saved as a draft that is not used until enabled. */
        LessonRequest: {
            /**
             * Format: uuid
             * @description Leave out for a lesson of the whole workspace.
             */
            dataset_id?: string | null;
            /**
             * Format: uuid
             * @description The error log row the lesson comes from, if any.
             */
            error_id?: string | null;
            /** @description One or two short sentences, at most 300 characters. */
            text: string;
        };
        /** @description Turn a lesson on (approve it, recording who and when) or off. */
        LessonStateRequest: {
            enabled: boolean;
        };
        /** @description Newest first. */
        LessonsResponse: {
            lessons: components["schemas"]["Lesson"][];
        };
        /** @description Multipart form for listing the sheets of an Excel file. The file is not stored. */
        ListSheetsRequest: {
            /**
             * Format: binary
             * @description One Excel `.xlsx` file.
             */
            file: string;
        };
        /** @description One call to the language model, in the order the calls finished. */
        LlmCall: {
            /**
             * Format: int64
             * @description Wall time from sending the request to reading the whole answer, in milliseconds.
             */
            duration_ms: number;
            /**
             * Format: int64
             * @description Prompt tokens as counted by the provider; null when it did not say (mock, an error, a cancelled call).
             */
            input_tokens: number | null;
            /** @description False when the provider returned an error, or when the caller stopped waiting (its own timeout). */
            ok: boolean;
            /**
             * Format: int64
             * @description Answer tokens as counted by the provider, thinking tokens included; null when it did not say.
             */
            output_tokens: number | null;
            task: components["schemas"]["LlmTask"];
        };
        /**
         * @description What the language model was asked to do in one call.
         * @enum {string}
         */
        LlmTask: "sql" | "repair" | "profile" | "narrate" | "why" | "insight" | "classify" | "judge";
        LoginRequest: {
            email: string;
            password: string;
        };
        MainMeasure: {
            /** @description The measure column, or null for a metric. */
            column: string | null;
            /** @description Metric name, or the column label (else name) of a measure column; also the measure column of the chart. */
            label: string;
            /** @description The metric of that name, or null for a measure column. */
            metric: string | null;
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
        /**
         * @description How the source rows make up a measure:
         *     - `sum`: `SUM(x)`; `row_column` holds x of each row and `value` is their sum.
         *     - `count`: `COUNT(*)`; each row counts one, so `value` equals `total_rows`.
         *     - `other`: any other aggregate (AVG, COUNT DISTINCT, a ratio, a rounded sum); `value` is that aggregate over the
         *       rows, there is no per-row column.
         * @enum {string}
         */
        MeasureRule: "sum" | "count" | "other";
        /** @description The full set of data roles of a member; empty removes every role. */
        MemberDataRolesRequest: {
            role_ids: string[];
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
        /** @description One column of a table kept as metadata only, read from the source catalog. */
        MetadataColumn: {
            /** @description Postgres type as `format_type` writes it, for example `numeric(10,2)`. */
            data_type: string;
            /** @description `COMMENT ON COLUMN`, cleaned; null when none. */
            description: string | null;
            name: string;
            nullable: boolean;
            primary_key: boolean;
        };
        /** @description A table of the source with columns, types, keys and comments but no rows. */
        MetadataOnlyTable: {
            cause: components["schemas"]["SkipCause"];
            columns: components["schemas"]["MetadataColumn"][];
            /**
             * Format: uuid
             * @description Dataset of an earlier snapshot of this table, which keeps its last version; null when never snapshotted.
             */
            dataset_id: string | null;
            /** @description `COMMENT ON TABLE`, cleaned; null when none. */
            description: string | null;
            /** @description Why, in Vietnamese, with the setting to change. */
            reason: string;
            /**
             * Format: int64
             * @description Rows estimated by Postgres, or counted when SOURCE_MAX_ROWS is set; null when Postgres has no statistics yet.
             */
            row_count: number | null;
            /**
             * Format: int64
             * @description Size reported by Postgres; null for views.
             */
            size_bytes: number | null;
            /** @description `schema.table`. */
            table: string;
        };
        /** @description A metric of the dataset: the version in use (the latest confirmed one). */
        Metric: {
            /** @description False for a draft nobody approved; the SQL prompt marks it as such. */
            confirmed: boolean;
            description: string;
            /** @description Aggregate SQL expression over the dataset's columns, for example `SUM("Order Item Total")`. */
            expression: string;
            /** @description Row condition applied with the expression, for example `"Order Status" <> 'CANCELED'`. */
            filter: string | null;
            /**
             * Format: uuid
             * @description Id of this version; editing or deleting by it acts on the metric with this name.
             */
            id: string;
            name: string;
            /** @description `ui` (written on the metrics screen), `sample`, `formula_table` or `llm_suggested`. */
            origin: string;
            /** @description Column that dates the rows for this metric. */
            time_column: string | null;
            unit: string | null;
            /**
             * Format: int32
             * @description Starts at 1; each edit adds a version and keeps the earlier ones.
             */
            version: number;
        };
        /** @description A column of the current version, for the column picker and the default aggregation of measure columns. */
        MetricColumn: {
            aggregation: components["schemas"]["ColumnAggregation"] | null;
            /** @description DuckDB type, for example `DOUBLE`, `BIGINT`, `VARCHAR`. */
            data_type: string;
            description: string | null;
            /**
             * @description Who wrote `description`: `database` (a `COMMENT ON COLUMN` of the source database) or `model` (the semantic
             *     profile); null when there is no description.
             */
            description_source: string | null;
            name: string;
            /** @description `dimension`, `measure`, `time`, `key`, `text` or `ignore`. */
            role: string;
        };
        /** @description A metric to create, or the new version of one. Errors name the wrong field in `field`. */
        MetricRequest: {
            description?: string;
            /** @description One aggregate SQL expression over the dataset's columns, column names in double quotes. */
            expression: string;
            filter?: string | null;
            /** @description Name users ask with, unique in the dataset ignoring case and accents. Cannot change when editing. */
            name: string;
            time_column?: string | null;
            unit?: string | null;
        };
        /** @description Metrics of a dataset and the columns of its current version. */
        MetricsResponse: {
            /** @description Columns of the current version in table order; empty while the dataset has no profiled version. */
            columns: components["schemas"]["MetricColumn"][];
            /** @description One entry per metric name, sorted by name. */
            metrics: components["schemas"]["Metric"][];
        };
        /**
         * @description What is missing:
         *     - `no_version`: the dataset has no data yet.
         *     - `profile_pending`: the column profile is being built; no columns or charts yet.
         *     - `profile_failed`: the profile failed; upload the file again.
         *     - `semantic_pending`: labels, descriptions and units are being written.
         *     - `semantic_unavailable`: the current model cannot write labels, descriptions and units.
         *     - `restricted`: the viewer's data role filters rows, so per-column counts and value charts are left out.
         *     - `no_time`: no time column, so no time span and no time charts.
         *     - `no_measure`: no confirmed metric and no money or quantity measure column, so no measure chart.
         *     - `chart_failed`: a chart query failed; the message says why.
         * @enum {string}
         */
        MissingCode: "no_version" | "profile_pending" | "profile_failed" | "semantic_pending" | "semantic_unavailable" | "restricted" | "no_time" | "no_measure" | "chart_failed";
        MissingPart: {
            code: components["schemas"]["MissingCode"];
            /** @description Text for the end user (Vietnamese). */
            message: string;
        };
        /**
         * @description Outcome of the number check (the backend compares every number and direction of the model's text with the computed
         *     facts): `passed` (the shown text is the model's and passed), `replaced` (the model's text failed and the shown text
         *     was written by the backend), `not_needed` (the backend wrote the text from the numbers, no model text to check).
         * @enum {string}
         */
        NumberCheck: "passed" | "replaced" | "not_needed";
        OverviewChart: {
            chart: components["schemas"]["ChartSuggestion"];
            /** @description Column of the profile the chart is about: the dimension for `rows_by_value`, the time column otherwise. */
            column: string;
            grain: components["schemas"]["TimeGrain"] | null;
            kind: components["schemas"]["OverviewChartKind"];
            result: components["schemas"]["ResultTable"];
            /** @description SQL after the guard that computed the chart; null for `rows_by_value`, which no query computes. */
            sql: string | null;
        };
        /**
         * @description Kind of pre-built chart:
         *     - `measure_over_time`: the main measure by day or month.
         *     - `rows_over_time`: number of rows by day or month, to see periods with little data or data that stops mid-period.
         *     - `rows_by_value`: number of rows for each value of a dimension column, read from the profile.
         * @enum {string}
         */
        OverviewChartKind: "measure_over_time" | "rows_over_time" | "rows_by_value";
        OverviewColumn: {
            description: string | null;
            /**
             * @description Who wrote `description`: `database` (a `COMMENT ON COLUMN` of the source database) or `model` (the semantic
             *     profile); null when there is no description.
             */
            description_source: string | null;
            /**
             * Format: int64
             * @description Exact number of different values; null when `summary.restricted`.
             */
            distinct_count: number | null;
            /** @description Vietnamese label: the first alias written by the semantic profile; null until then. */
            label: string | null;
            name: string;
            /**
             * Format: double
             * @description Share of empty cells, 0 to 100; null when `summary.restricted`.
             */
            null_percent: number | null;
            role: components["schemas"]["OverviewRole"];
            /** @description Marked sensitive: no values are returned for it. */
            sensitive: boolean;
            /** @description money, quantity, percent, duration or other; null when unknown. */
            unit: string | null;
            /** @description Values with their row counts, most rows first; only for dimension columns that are not sensitive, at most 200. */
            values: components["schemas"]["OverviewValue"][];
        };
        OverviewMetric: {
            /** @description False while an admin has not confirmed the definition. */
            confirmed: boolean;
            description: string;
            /** @description Aggregate over the columns, for example `SUM("Order Item Total")`. */
            expression: string;
            filter: string | null;
            name: string;
            time_column: string | null;
            unit: string | null;
        };
        /**
         * @description Role of a column in the profile; the rules are in docs/DATA.md.
         * @enum {string}
         */
        OverviewRole: "dimension" | "measure" | "time" | "key" | "text";
        OverviewSummary: {
            /**
             * @description True when the viewer's data role filters rows. Per-column counts and value frequencies are computed on every
             *     row, so they are left out: null `null_percent` and `distinct_count`, empty `values`, no `rows_by_value` charts.
             */
            restricted: boolean;
            /**
             * Format: int64
             * @description Rows the viewer can read, after the row filters of their data role; null when not counted yet or the count failed.
             */
            row_count: number | null;
            /** @description Time column of the time span and the time charts; null when the dataset has no time column. */
            time_column: string | null;
            time_end: string | null;
            /** @description First and last day with data in `time_column` (YYYY-MM-DD), among the rows the viewer can read. */
            time_start: string | null;
        };
        OverviewValue: {
            /** @description First alias of the value; null when it has none. */
            label: string | null;
            /**
             * Format: int64
             * @description Rows holding this value, on every row of the version.
             */
            rows: number;
            value: string;
        };
        OverviewVersion: {
            /** Format: date-time */
            created_at: string;
            /** Format: uuid */
            id: string;
            /**
             * Format: int32
             * @description Version number, from 1.
             */
            number: number;
            /** @description pending, structure, semantic or failed (see `Dataset.current_profile_status`). */
            profile_status: string;
        };
        /** @description The stored rating of an answer. */
        QueryFeedback: {
            /**
             * @description An admin approved the answer as an example question: its question and SQL are now examples in the prompt of
             *     later questions on the same dataset that share a word with it.
             */
            approved: boolean;
            note: string | null;
            /** Format: date-time */
            rated_at: string;
            rating: components["schemas"]["Rating"];
            reason: components["schemas"]["FeedbackReason"] | null;
        };
        /**
         * @description Kind of question by the purpose of the analysis, as the classify step decided: `quantitative` (a number, a list or
         *     a comparison, answered by one SQL query) or `causal` (why a number changed or differs between two groups, answered by
         *     a breakdown).
         * @enum {string}
         */
        QuestionType: "quantitative" | "causal";
        /**
         * @description `good` is the button for a good answer, `bad` the one for an answer that falls short.
         * @enum {string}
         */
        Rating: "good" | "bad";
        /** @description A dataset linked to this one by a foreign key of the source database. */
        RelatedDataset: {
            /** @description Key columns of this dataset. */
            columns: string[];
            /** Format: uuid */
            dataset_id: string;
            /** @description `many_to_one` when this dataset holds the foreign key, `one_to_many` when the related dataset holds it. */
            direction: string;
            name: string;
            /** @description Key columns of the related dataset, in the same order. */
            related_columns: string[];
        };
        /** @description A table linked to the chosen table by one foreign key. */
        RelatedTable: {
            /** @description True when `table` can be snapshotted with the chosen table and the relation recorded. */
            addable: boolean;
            /** @description Readable columns of `table`; 0 when the account cannot read it. */
            column_count: number;
            constraint: string;
            from_columns: string[];
            /** @description Table holding the foreign key (the many side) and its key columns. */
            from_table: string;
            /** @description Why the table cannot be added, in Vietnamese; null when addable. */
            reason: string | null;
            /** @description `schema.table` that can be snapshotted together with the chosen table. */
            table: string;
            to_columns: string[];
            /** @description Table the foreign key points to (the one side) and its key columns. */
            to_table: string;
        };
        RelationsRequest: {
            /** @description `schema.table` from the table list. */
            table: string;
        };
        RelationsResponse: {
            /** @description Readable columns of the chosen table. */
            column_count: number;
            /** @description Tables the chosen table points to (its foreign keys). */
            many_to_one: components["schemas"]["RelatedTable"][];
            /** @description Tables pointing to the chosen table (their foreign keys). */
            one_to_many: components["schemas"]["RelatedTable"][];
            table: string;
        };
        /** @description Every item id of the report, in the new order. */
        ReorderReportRequest: {
            item_ids: string[];
        };
        /**
         * @description The breakdown kept with an item: the change between two periods or two groups and the groups of its first dimension
         *     that made most of it. Numbers are decimal strings as in `Explanation`.
         */
        ReportBreakdown: {
            base_label: string;
            base_value: string;
            change: string;
            compare_label: string;
            compare_value: string;
            /** @description Label of the dimension the groups belong to. */
            dimension: string;
            groups: components["schemas"]["ReportBreakdownGroup"][];
            measure: string;
        };
        ReportBreakdownGroup: {
            base_value: string;
            compare_value: string;
            contribution: string;
            /** @description Group value as text; empty for a missing value. */
            name: string;
            /** @description Share of the total change, decimal string; null when the change is zero. */
            share_of_change: string | null;
        };
        /**
         * @description Facts the backend computed from the numbers of the item when it was rendered (no language model): the main finding
         *     and its points, Vietnamese.
         */
        ReportFacts: {
            points: string[];
            title: string;
        };
        /**
         * @description One item of a report, rendered again: the stored question and answer, and the result of running its stored SQL
         *     again through the guard with the caller's access, on the data versions the question read.
         */
        ReportItem: {
            answer: components["schemas"]["SavedAnswer"] | null;
            /** Format: date-time */
            asked_at: string;
            breakdown: components["schemas"]["ReportBreakdown"] | null;
            chart: components["schemas"]["ChartSuggestion"] | null;
            /** @description Vietnamese message when the stored SQL could not run again; the other result fields are then null. */
            error: string | null;
            facts: components["schemas"]["ReportFacts"] | null;
            /** Format: uuid */
            id: string;
            meaning_check: components["schemas"]["AnswerVerdict"] | null;
            /**
             * @description Vietnamese notes on how this rendering differs from when the question was asked (row count, newer table
             *     version, fewer visible rows).
             */
            notes: string[];
            number_check: components["schemas"]["NumberCheck"] | null;
            /** Format: uuid */
            query_id: string;
            question: string;
            question_type: components["schemas"]["QuestionType"];
            result: components["schemas"]["ResultTable"] | null;
            /** @description SQL that ran now, after the guard. */
            sql: string | null;
            /** @description Tables and versions the question read, the starting table first. */
            tables: components["schemas"]["ReportTableVersion"][];
            understanding: components["schemas"]["Understanding"] | null;
        };
        /**
         * @description `manual`: answers the user added one by one; `automatic`: built by the backend for one dataset.
         * @enum {string}
         */
        ReportKind: "manual" | "automatic";
        /** @description Reports of the caller, last changed first. */
        ReportListResponse: {
            reports: components["schemas"]["ReportSummary"][];
        };
        /** @description A report with every item rendered again from the stored questions. */
        ReportResponse: {
            items: components["schemas"]["ReportItem"][];
            /**
             * Format: date-time
             * @description When the items were run again; numbers of every item are as of this time, on the stored data versions.
             */
            rendered_at: string;
            report: components["schemas"]["ReportSummary"];
        };
        /** @description One report of the caller, without its items. */
        ReportSummary: {
            /** Format: date-time */
            created_at: string;
            /**
             * Format: uuid
             * @description Dataset of an automatic report (`DatasetSummary.id`); null for a manual report.
             */
            dataset_id: string | null;
            /** Format: uuid */
            id: string;
            /** @description Opening paragraph written by the owner (Vietnamese); empty when none. */
            intro: string;
            /** Format: int32 */
            item_count: number;
            kind: components["schemas"]["ReportKind"];
            /**
             * Format: int32
             * @description Language model calls made while building an automatic report; null for a manual report.
             */
            model_calls: number | null;
            title: string;
            /** Format: date-time */
            updated_at: string;
        };
        /** @description Table and data version an item read. */
        ReportTableVersion: {
            /**
             * Format: date-time
             * @description When this version was snapshotted or uploaded.
             */
            captured_at: string;
            /** @description False when the table has a newer version: the item still reads this one. */
            is_current: boolean;
            name: string;
            /** Format: uuid */
            table_id: string;
            /** Format: int32 */
            version: number;
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
        /**
         * @description A row rule of a data role: members with this role see only the rows of table `table_id` whose `column` is one of
         *     `allowed_values`.
         */
        RolePolicy: {
            /** @description Values of `column` the role sees; empty means no rows of the table. */
            allowed_values: string[];
            /** @description A dimension column of the table. */
            column: string;
            /**
             * Format: uuid
             * @description Table id (`DatasetTable.id` in the catalog).
             */
            table_id: string;
        };
        /** @description A plausibility rule for values of the concept. `min` and `max` only with `value_range`, at least one of them. */
        SanityCheck: {
            kind: components["schemas"]["SanityCheckKind"];
            /** Format: double */
            max?: number | null;
            /** Format: double */
            min?: number | null;
        };
        /**
         * @description `value_range`: the value lies in [min, max], one end may be missing. `non_negative`: the value is not negative.
         * @enum {string}
         */
        SanityCheckKind: "value_range" | "non_negative";
        /**
         * @description The answer text of a recorded question, as shown when it was asked: `answer_text` of a plain question, or
         *     `explanation.summary` of a "why" question. Result rows are not stored.
         */
        SavedAnswer: {
            /** @description True when a language model wrote the text and it passed the checks; false when the backend wrote it. */
            by_model: boolean;
            /** @description Vietnamese text for the end user. */
            text: string;
            verdict: components["schemas"]["AnswerVerdict"] | null;
            /** @description Points the check found unclear in the model's text; empty when `by_model` is false. */
            warnings: string[];
        };
        /** @description The full rule set of a role; replaces the previous one. */
        SetPoliciesRequest: {
            /** @description One rule per table and column. Each `column` must be a dimension column of the table's current version. */
            policies: components["schemas"]["RolePolicy"][];
        };
        SetPoliciesResponse: {
            /** @description Rules as saved: values trimmed, without duplicates, sorted. */
            policies: components["schemas"]["RolePolicy"][];
            /** @description Vietnamese notes for the admin, for example a value not found in the column profile. The rules are saved. */
            warnings: string[];
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
         * @description Why a table keeps metadata only.
         * @enum {string}
         */
        SkipCause: "row_limit" | "size_limit";
        /**
         * @description `base` or `compare`: one of the two slices of `AskResponse.explanation`.
         * @enum {string}
         */
        SliceName: "base" | "compare";
        SnapshotRequest: {
            /**
             * Format: uuid
             * @description Snapshot this table (a `datasets.id`, the field name predates tables) again, with every table of the same source
             *     linked to it by relations.
             */
            dataset_id?: string | null;
            /** @description Name stored for `table` when it is new; defaults to the table name. Other tables use their table name. */
            name?: string | null;
            /**
             * @description `schema.table` named `name` when it is new. Optional: without `table`, `tables` and `dataset_id`, every table
             *     and view the account can read is snapshotted.
             */
            table?: string | null;
            /**
             * @description `schema.table` names to snapshot (the ticked tables), each stored as one table of the database dataset. Empty
             *     with no `table` and no `dataset_id`: every readable table. One name snapshots that one table, for example a table kept as metadata
             *     only. Not allowed with `dataset_id`.
             */
            tables?: string[];
        };
        SnapshotResponse: {
            /** @description One entry per table copied. `created` is false for a table whose content did not change. */
            datasets: components["schemas"]["UploadResult"][];
            /**
             * @description Tables over SOURCE_TABLE_MAX_MB, the SOURCE_SNAPSHOT_MAX_MB total, or SOURCE_MAX_ROWS when set: no rows copied,
             *     metadata kept.
             */
            metadata_only: components["schemas"]["MetadataOnlyTable"][];
            /** @description Foreign-key relations recorded for these datasets (both ends snapshotted from this source). */
            relations: components["schemas"]["DatasetRelation"][];
            /**
             * @description Tables this source held that the account can no longer read (re-snapshot only); their datasets keep their
             *     last version.
             */
            unreadable: string[];
        };
        /** @description A database source without its password. */
        SourceResponse: {
            /** Format: date-time */
            created_at: string;
            /** @description True when the source was added with its own CA certificate. */
            custom_ca: boolean;
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
        /** @description A table or view the source account can read, with what the connect screen needs to pick tables. */
        SourceTableInfo: {
            /**
             * Format: int64
             * @description Columns the account can read.
             */
            column_count: number;
            /** @description `COMMENT ON TABLE`, cleaned; null when none. */
            comment: string | null;
            /**
             * Format: uuid
             * @description Dataset of the table when `state` is `snapshotted`.
             */
            dataset_id: string | null;
            /**
             * Format: int64
             * @description Row estimate from Postgres statistics (last ANALYZE); null when Postgres has none yet, and for views.
             */
            estimated_rows: number | null;
            /**
             * @description True when another readable table points to this one (a lookup table): unticking it leaves those tables without
             *     the relation.
             */
            is_referenced: boolean;
            /** @description `table` or `view`. */
            kind: string;
            name: string;
            over_limit: components["schemas"]["SkipCause"] | null;
            /** @description Why the table has metadata only; null otherwise. */
            reason: string | null;
            /** @description Foreign keys of other readable tables pointing to this table. */
            referenced_by: components["schemas"]["TableLink"][];
            /** @description Foreign keys of this table pointing to other readable tables. */
            references: components["schemas"]["TableLink"][];
            schema: string;
            /**
             * Format: int64
             * @description Size reported by Postgres (`pg_table_size`, without indexes); null for views.
             */
            size_bytes: number | null;
            state: components["schemas"]["TableState"] | null;
            /** @description `schema.table`, the name to send in `SnapshotRequest.tables`. */
            table: string;
        };
        /**
         * @description Step of the backend:
         *     - `context`: read the dataset profile and build the prompt.
         *     - `classify`: the language model tells a plain question from a question that asks for a cause; when the model
         *       fails, the keyword rule decides and the step is not ok.
         *     - `llm`: the language model writes SQL.
         *     - `guard`: check the SQL, add row-level filters and a row limit.
         *     - `run`: run the SQL on the analytics engine.
         *     - `repair`: the language model fixes SQL that failed to run, or that the guard refused because a table or column
         *       does not exist or because it could build a value too large (at most once).
         *     - `explain`: compute the breakdown of a "why" question (code, no model).
         *     - `narrate`: the language model writes the summary of the breakdown, or the `insight.summary` of a plain question
         *       (also used as its `answer_text`).
         *     - `record`: save the question in the history.
         * @enum {string}
         */
        StepName: "context" | "classify" | "llm" | "guard" | "run" | "repair" | "explain" | "narrate" | "record";
        /** @description One point of the summary: a short lead (shown in bold) and the sentence after it. */
        SummaryPoint: {
            kind: components["schemas"]["SummaryPointKind"];
            /** @description Lead of the point (Vietnamese), for example "Lý do chính". */
            lead: string;
            /** @description Rest of the point (Vietnamese). */
            text: string;
        };
        /**
         * @description - `reason`: what mostly makes the change (the volume and the value per unit, or the mix and the rate); every point
         *       written by the model has this kind.
         *     - `outliers`: groups that changed much more or much less than the whole, or exist on one side only.
         *     - `even`: no group stands out; the largest contributions are the largest groups.
         *     - `largest`: the group with the largest contribution in the direction of the change, when the numbers do not allow
         *       comparing groups with the whole (a base slice of zero or less, or a change too close to zero).
         *     - `against`: a group that moved the other way.
         *     - `dropped`: dimensions the summary leaves out and why; they stay in `Explanation.dimensions`.
         * @enum {string}
         */
        SummaryPointKind: "reason" | "outliers" | "even" | "largest" | "against" | "dropped";
        /** @description A column of the current version of a table. */
        TableColumn: {
            /** @description Other names of the column (semantic profile), used to match question words. */
            aliases: string[];
            /** @description DuckDB type, for example `BIGINT`, `VARCHAR`, `DATE`. */
            data_type: string;
            description: string | null;
            /** @description `database` (comment in the source database) or `model` (written by the semantic profile); null when unknown. */
            description_source: string | null;
            /** @description Sensitive column: its values never go to the language model. */
            is_sensitive: boolean;
            name: string;
            /** @description `dimension`, `measure`, `time`, `key`, `text` or `ignore`. */
            role: string;
        };
        /** @description A foreign key between two readable tables, as declared in the source database. */
        TableLink: {
            constraint: string;
            from_columns: string[];
            /** @description Table holding the foreign key (the many side) and its key columns. */
            from_table: string;
            to_columns: string[];
            /** @description Table the foreign key points to (the one side) and its key columns. */
            to_table: string;
        };
        /** @description A dataset the SQL of an answer read. */
        TableRead: {
            /** Format: uuid */
            dataset_id: string;
            /** @description Table name the SQL uses for this dataset. */
            logical_table: string;
            /** @description Dataset name as shown in the dataset list. */
            name: string;
        };
        /**
         * @description What a source holds for one table.
         * @enum {string}
         */
        TableState: "snapshotted" | "metadata_only";
        TestSourceResponse: {
            /** @description Number of tables and views the account can read. */
            table_count: number;
        };
        /**
         * @description Period of the time charts: by day when the data spans at most 92 days, else by month.
         * @enum {string}
         */
        TimeGrain: "day" | "month";
        /** @description One recorded question of a conversation. Questions waiting for a choice (`needs_choice`) are not recorded. */
        TurnItem: {
            answer: components["schemas"]["SavedAnswer"] | null;
            /** Format: date-time */
            created_at: string;
            feedback: components["schemas"]["QueryFeedback"] | null;
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
        /** @description Fields left out keep their value. */
        UpdateReportRequest: {
            intro?: string | null;
            title?: string | null;
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
    listCatalog: {
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
                    "application/json": components["schemas"]["DatasetSummary"][];
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
    getCatalogDataset: {
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
                    "application/json": components["schemas"]["DatasetSummary"];
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
    askCatalogDataset: {
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
                "application/json": components["schemas"]["DatasetAskRequest"];
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
            /** @description The language model service failed before any SQL was written */
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
            /** @description 400 empty question, a dataset with no table or no data yet; 401; 404 not a member, no such dataset, a `table_id` outside the dataset, or a conversation that is not the caller's */
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
    listCatalogConversations: {
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
            /** @description 401; 404 not a member or no such dataset */
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
    snapshotCatalogDataset: {
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
            /** @description No table changed: nothing created */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SnapshotResponse"];
                };
            };
            /** @description At least one table got a new version */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SnapshotResponse"];
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
            /** @description 400 a file dataset, no table yet, or the database refused; 401; 403 not admin; 404 not a member or no such dataset; 413 a copied table bigger than MAX_UPLOAD_MB */
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
    listDataRoles: {
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
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DataRoleResponse"][];
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
            /** @description 401; 403 not admin; 404 not a member */
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
    createDataRole: {
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
                "application/json": components["schemas"]["DataRoleRequest"];
            };
        };
        responses: {
            /** @description Created */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DataRoleRow"];
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
            /** @description 400 empty, too long or taken name; 401; 403 not admin; 404 not a member */
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
    listMyDataRoles: {
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
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DataRoleRow"][];
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
            /** @description 401; 404 not a member */
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
    deleteDataRole: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Data role id */
                role_id: string;
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
            /** @description 400 still held by members; 401; 403 not admin; 404 not a member or no such role */
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
    updateDataRole: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Data role id */
                role_id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["DataRoleRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DataRoleRow"];
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
            /** @description 400 empty, too long or taken name; 401; 403 not admin; 404 not a member or no such role */
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
    setDataRolePolicies: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Data role id */
                role_id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SetPoliciesRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SetPoliciesResponse"];
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
            /** @description 400 unknown table or column, not a dimension column, two rules on one column, same rules as another role, or no rules left on a role held by members; 401; 403 not admin; 404 not a member or no such role */
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
    setColumnAggregation: {
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
                "application/json": components["schemas"]["ColumnAggregationRequest"];
            };
        };
        responses: {
            /** @description Saved */
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
            /** @description 400 no measure column of that name (`field` is `column`); 401; 403 not admin; 404 no such dataset */
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
    exportExamples: {
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
                    "application/json": components["schemas"]["EvalCaseCandidate"][];
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
            /** @description 401; 403 not admin; 404 not a member or invalid dataset id */
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
    listFilterColumns: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Table id (`DatasetTable.id`) */
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
                    "application/json": components["schemas"]["FilterColumnResponse"][];
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
            /** @description 400 unknown table or no data yet; 401; 403 not admin; 404 not a member */
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
    listMetrics: {
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
                    "application/json": components["schemas"]["MetricsResponse"];
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
            /** @description 401; 404 not a member or no such dataset */
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
    createMetric: {
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
                "application/json": components["schemas"]["MetricRequest"];
            };
        };
        responses: {
            /** @description Created */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Metric"];
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
            /** @description 400 invalid metric, `field` names the field; 401; 403 not admin; 404 not a member or no such dataset */
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
    updateMetric: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Dataset id (UUID) */
                id: string;
                /** @description Id of any version of the metric (UUID) */
                metric_id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["MetricRequest"];
            };
        };
        responses: {
            /** @description The new version */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Metric"];
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
            /** @description 400 invalid metric, `field` names the field; 401; 403 not admin; 404 no such dataset or metric */
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
    deleteMetric: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Dataset id (UUID) */
                id: string;
                /** @description Id of any version of the metric (UUID) */
                metric_id: string;
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
            /** @description 401; 403 not admin; 404 no such dataset or metric */
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
    getDatasetOverview: {
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
                    "application/json": components["schemas"]["DatasetOverviewResponse"];
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
    listConcepts: {
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
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ConceptsResponse"];
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
            /** @description 401; 404 not a member */
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
    createConcept: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Pack key, for example `retail` */
                pack: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ConceptRequest"];
            };
        };
        responses: {
            /** @description Created */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Concept"];
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
            /** @description 400 invalid concept, `field` names the field; 401; 403 not admin; 404 not a member or no such pack */
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
    updateConcept: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Pack key, for example `retail` */
                pack: string;
                /** @description Concept id (UUID) */
                concept_id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ConceptRequest"];
            };
        };
        responses: {
            /** @description Saved */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Concept"];
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
            /** @description 400 invalid concept, `field` names the field; 401; 403 not admin; 404 no such pack or concept */
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
    listErrorLog: {
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
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorLogResponse"];
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
            /** @description 401; 403 not admin; 404 not a member */
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
    listExamples: {
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
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ExampleCandidatesResponse"];
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
            /** @description 401; 403 not admin; 404 not a member */
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
    approveExample: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description `query_id` of the answer (UUID) */
                query_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Approved */
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
            /** @description 401; 403 not admin; 404 not a member, or no answered question rated good with this id */
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
    withdrawExample: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description `query_id` of the answer (UUID) */
                query_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Withdrawn */
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
            /** @description 401; 403 not admin; 404 not a member, or no answered question rated good with this id */
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
    listLessons: {
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
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["LessonsResponse"];
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
            /** @description 401; 403 not admin; 404 not a member */
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
    createLesson: {
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
                "application/json": components["schemas"]["LessonRequest"];
            };
        };
        responses: {
            /** @description Created as a draft */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Lesson"];
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
            /** @description 400 empty or over 300 characters, or a dataset or error log row not in this workspace; 401; 403 not admin; 404 not a member */
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
    deleteLesson: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Lesson id (UUID) */
                lesson_id: string;
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
            /** @description 401; 403 not admin; 404 not a member or no such lesson */
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
    setLessonState: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Lesson id (UUID) */
                lesson_id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["LessonStateRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Lesson"];
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
            /** @description 401; 403 not admin; 404 not a member or no such lesson */
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
    setMemberDataRoles: {
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
                "application/json": components["schemas"]["MemberDataRolesRequest"];
            };
        };
        responses: {
            /** @description Saved */
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
            /** @description 400 a role not in this workspace or without rules; 401; 403 not admin; 404 not a member */
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
    rateAnswer: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description `query_id` of the answer (UUID) */
                query_id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["FeedbackRequest"];
            };
        };
        responses: {
            /** @description The stored rating */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["QueryFeedback"];
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
            /** @description 400 a reason or note with `good`, or a note over 500 characters; 401; 404 not a member, or no such answer of the caller */
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
    clearRating: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description `query_id` of the answer (UUID) */
                query_id: string;
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
            /** @description 401; 404 not a member, or no such answer of the caller */
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
    listReports: {
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
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ReportListResponse"];
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
            /** @description 401; 404 not a member */
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
    createReport: {
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
                "application/json": components["schemas"]["CreateReportRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ReportSummary"];
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
            /** @description 400 empty title, title over 200 or intro over 5000 characters; 401; 404 not a member */
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
    createAutomaticReport: {
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
                "application/json": components["schemas"]["AutoReportRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AutoReportResponse"];
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
            /** @description 400 the dataset has no measure, or no section could be built; 401; 404 not a member or no such dataset */
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
    getReport: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Report id (UUID) */
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
                    "application/json": components["schemas"]["ReportResponse"];
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
            /** @description 401; 404 not a member, or no such report of the caller */
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
    deleteReport: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Report id (UUID) */
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
            /** @description 401; 404 not a member, or no such report of the caller */
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
    updateReport: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Report id (UUID) */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["UpdateReportRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ReportSummary"];
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
            /** @description 400 empty or long title, long intro; 401; 404 not a member, or no such report of the caller */
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
    addReportItem: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Report id (UUID) */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AddReportItemRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ReportSummary"];
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
            /** @description 400 the question was not answered, is already in the report, or the report has 30 items; 401; 404 not a member, no such report of the caller or no such question of the caller */
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
    removeReportItem: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Report id (UUID) */
                id: string;
                /** @description Item id (`ReportItem.id`, UUID) */
                item_id: string;
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
            /** @description 401; 404 not a member, no such report of the caller or no such item */
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
    reorderReportItems: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Report id (UUID) */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ReorderReportRequest"];
            };
        };
        responses: {
            /** @description Order saved */
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
            /** @description 400 the list is not exactly every item of the report; 401; 404 not a member, or no such report of the caller */
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
    exportReportXlsx: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Workspace slug */
                ws: string;
                /** @description Report id (UUID) */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The workbook */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet": unknown;
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
            /** @description 401; 404 not a member, or no such report of the caller */
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
            /** @description No table created anything (unchanged or metadata only) */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SnapshotResponse"];
                };
            };
            /** @description At least one new dataset or version created */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SnapshotResponse"];
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
            /** @description 400 unknown table, cannot read it or cannot connect, 401, 403 not admin, 404, 413 a copied view (no size known before copying) is bigger than SOURCE_TABLE_MAX_MB */
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
                    "application/json": components["schemas"]["SourceTableInfo"][];
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
