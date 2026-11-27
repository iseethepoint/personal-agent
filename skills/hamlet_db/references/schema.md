# Hamlet database schema

Tables readable by `hamlet.db`. Always use the schema-qualified name.

## `agent.agents`

| column | type | nullable | default |
|---|---|---|---|
| `agent_id` | text | no |  |
| `id` | text | yes |  |
| `session_id` | text | yes |  |
| `kind` | text | no | `'root'::text` |
| `parent_id` | text | yes |  |
| `parent_agent_id` | text | yes |  |
| `root_session_id` | text | yes |  |
| `model` | text | no | `''::text` |
| `status` | text | no |  |
| `depth` | integer | no | `0` |
| `created_at` | bigint | no | `(EXTRACT(epoch FROM now()))::bigint` |
| `updated_at` | bigint | no | `(EXTRACT(epoch FROM now()))::bigint` |
| `last_assistant_message` | text | yes |  |
| `seen_at_ms` | bigint | yes |  |
| `prompt_floor_seq` | bigint | yes |  |
| `ephemeral` | boolean | no | `false` |
| `agent_type` | text | yes |  |
| `spawn_metadata_json` | text | yes |  |

- UNIQUE `id` (`agents_id_key`)
- PRIMARY KEY `agent_id` (`agents_pkey`)

## `agent.context_items`

Rows holding model reasoning (thinking and redacted-thinking items) are withheld. Stored commentary can be queried through `hamlet.db` for diagnosis. Commentary is hidden from the user's chat. A commentary record does not establish that the user received its contents.

| column | type | nullable | default |
|---|---|---|---|
| `context_item_id` | bigint | no | `nextval('context_items_context_item_id_seq'::regclass)` |
| `agent_id` | text | no |  |
| `seq` | bigint | no |  |
| `item_kind` | context_item_kind | no |  |
| `message_id` | text | yes |  |
| `role` | runtime.message_role | yes |  |
| `call_id` | text | yes |  |
| `tool_name` | text | yes |  |
| `success` | boolean | yes |  |
| `message_source` | text | yes |  |
| `created_at` | timestamp with time zone | no | `now()` |
| `text_content` | text | yes |  |
| `item_json` | text | yes |  |

- UNIQUE `agent_id` (`context_items_agent_id_seq_key`)
- UNIQUE `seq` (`context_items_agent_id_seq_key`)
- PRIMARY KEY `context_item_id` (`context_items_pkey`)

## `agent.volatile_context_pins`

| column | type | nullable | default |
|---|---|---|---|
| `agent_id` | text | no |  |
| `variant_hash` | text | no |  |
| `variant_json` | text | no |  |
| `prompt_floor_seq` | bigint | no |  |
| `checkpoint_seq` | bigint | no |  |
| `body` | text | yes |  |
| `created_at` | timestamp with time zone | no | `now()` |
| `body_hash` | bytea | yes |  |

- PRIMARY KEY `agent_id` (`volatile_context_pins_pkey`)
- PRIMARY KEY `variant_hash` (`volatile_context_pins_pkey`)

## `goals.briefings`

| column | type | nullable | default |
|---|---|---|---|
| `briefing_id` | text | no |  |
| `goal_id` | text | no |  |
| `path` | text | no |  |
| `created_at` | text | no |  |
| `updated_at` | text | no |  |
| `last_opened_at` | text | yes |  |
| `created_at_ts` | timestamp with time zone | yes |  |
| `updated_at_ts` | timestamp with time zone | yes |  |
| `last_opened_at_ts` | timestamp with time zone | yes |  |
| `hero_image_path` | text | yes |  |

- FOREIGN KEY `goal_id` → `goals.goals.goal_id`
- UNIQUE `goal_id` (`briefings_goal_id_path_key`)
- UNIQUE `path` (`briefings_goal_id_path_key`)
- PRIMARY KEY `briefing_id` (`briefings_pkey`)

## `goals.goals`

| column | type | nullable | default |
|---|---|---|---|
| `goal_id` | text | no |  |
| `slug` | text | yes |  |
| `title` | text | no |  |
| `summary` | text | yes |  |
| `description` | text | yes |  |
| `momentum` | text | yes |  |
| `momentum_status` | text | yes |  |
| `emoji` | text | yes |  |
| `status` | text | no |  |
| `created_at` | text | no |  |
| `updated_at` | text | no |  |
| `parent_goal_id` | text | yes |  |
| `sort_order` | double precision | no | `0` |
| `completed_at` | text | yes |  |
| `created_at_ts` | timestamp with time zone | yes |  |
| `updated_at_ts` | timestamp with time zone | yes |  |
| `last_activity_at` | text | yes |  |
| `last_activity_at_ts` | timestamp with time zone | yes |  |
| `pushable` | boolean | no | `true` |
| `category` | text | yes |  |
| `goal_kind` | text | yes |  |
| `value_alignment` | text | yes |  |
| `completed_at_ts` | timestamp with time zone | yes |  |
| `source` | text | no | `'user_goal'::text` |
| `attention_kind` | text | no | `'does_not_need_attention'::text` |
| `attention_updated_at` | timestamp with time zone | no | `now()` |
| `next_due_at` | timestamp with time zone | yes |  |
| `fixed_position` | bigint | yes |  |
| `escalation` | jsonb | yes |  |
| `escalation_due_at` | timestamp with time zone | yes |  |

- FOREIGN KEY `parent_goal_id` → `goals.goals.goal_id`
- PRIMARY KEY `goal_id` (`goals_pkey`)
- UNIQUE `slug` (`goals_slug_key`)

## `goals.updates`

| column | type | nullable | default |
|---|---|---|---|
| `update_id` | text | no |  |
| `goal_id` | text | no |  |
| `title` | text | yes |  |
| `summary` | text | yes |  |
| `description` | text | yes |  |
| `effective_at` | text | yes |  |
| `created_at` | text | no |  |
| `updated_at` | text | no |  |
| `effective_at_ts` | timestamp with time zone | yes |  |
| `updated_at_ts` | timestamp with time zone | yes |  |
| `author_source` | text | no | `'user'::text` |

- FOREIGN KEY `goal_id` → `goals.goals.goal_id`
- PRIMARY KEY `update_id` (`updates_pkey`)

## `memory.claims`

| column | type | nullable | default |
|---|---|---|---|
| `claim_id` | text | no |  |
| `run_id` | text | no |  |
| `kind` | text | no |  |
| `salience` | text | no |  |
| `claim_text` | text | no |  |
| `quote` | text | yes |  |
| `speaker` | text | no |  |
| `evidence_handles` | jsonb | no | `'[]'::jsonb` |
| `supersedes_claim_id` | text | yes |  |
| `status` | text | no | `'active'::text` |
| `confidence` | double precision | no |  |
| `first_seen` | timestamp with time zone | no | `now()` |
| `reinforced_at` | timestamp with time zone | no | `now()` |
| `valid_until` | timestamp with time zone | yes |  |
| `source_path` | text | no |  |
| `source_line` | bigint | no |  |
| `created_at` | timestamp with time zone | no | `now()` |
| `updated_at` | timestamp with time zone | no | `now()` |

- PRIMARY KEY `claim_id` (`claims_pkey`)

## `memory.embedding_models`

| column | type | nullable | default |
|---|---|---|---|
| `embedding_model_id` | bigint | no | `nextval('memory.embedding_models_embedding_model_id_seq'::regclass)` |
| `model_name` | text | no |  |
| `dimensions` | integer | no |  |
| `distance_metric` | text | no |  |
| `created_at` | timestamp with time zone | no | `now()` |

- UNIQUE `model_name` (`embedding_models_model_name_dimensions_distance_metric_key`)
- UNIQUE `dimensions` (`embedding_models_model_name_dimensions_distance_metric_key`)
- UNIQUE `distance_metric` (`embedding_models_model_name_dimensions_distance_metric_key`)
- PRIMARY KEY `embedding_model_id` (`embedding_models_pkey`)

## `memory.embeddings`

| column | type | nullable | default |
|---|---|---|---|
| `memory_embedding_id` | bigint | no | `nextval('memory.embeddings_memory_embedding_id_seq'::regclass)` |
| `memory_entry_id` | bigint | no |  |
| `embedding_model_id` | bigint | no |  |
| `embedding` | vector(384) | no |  |
| `created_at` | timestamp with time zone | no | `now()` |

- FOREIGN KEY `embedding_model_id` → `memory.embedding_models.embedding_model_id`
- UNIQUE `memory_entry_id` (`embeddings_memory_entry_id_embedding_model_id_key`)
- UNIQUE `embedding_model_id` (`embeddings_memory_entry_id_embedding_model_id_key`)
- FOREIGN KEY `memory_entry_id` → `memory.entries.memory_entry_id`
- PRIMARY KEY `memory_embedding_id` (`embeddings_pkey`)

## `memory.entries`

| column | type | nullable | default |
|---|---|---|---|
| `memory_entry_id` | bigint | no | `nextval('memory.entries_memory_entry_id_seq'::regclass)` |
| `memory_uri` | text | no |  |
| `chunk_id` | text | no |  |
| `source_type` | text | no |  |
| `status` | text | no |  |
| `privacy_class` | text | no |  |
| `confidence` | double precision | no | `1.0` |
| `citation_path` | text | yes |  |
| `line_start` | bigint | no | `0` |
| `line_end` | bigint | no | `0` |
| `created_at` | timestamp with time zone | no | `now()` |
| `created_at_unix` | bigint | no | `0` |
| `title_text` | text | yes |  |
| `body_text` | text | no |  |
| `reason_text` | text | yes |  |

- UNIQUE `memory_uri` (`entries_memory_uri_key`)
- PRIMARY KEY `memory_entry_id` (`entries_pkey`)

## `memory.entry_attributes`

| column | type | nullable | default |
|---|---|---|---|
| `memory_entry_attribute_id` | bigint | no | `nextval('memory.entry_attributes_memory_entry_attribute_id_seq'::regclass)` |
| `memory_entry_id` | bigint | no |  |
| `attribute_name` | text | no |  |
| `scalar_value` | text | no |  |

- UNIQUE `memory_entry_id` (`entry_attributes_memory_entry_id_attribute_name_key`)
- UNIQUE `attribute_name` (`entry_attributes_memory_entry_id_attribute_name_key`)
- FOREIGN KEY `memory_entry_id` → `memory.entries.memory_entry_id`
- PRIMARY KEY `memory_entry_attribute_id` (`entry_attributes_pkey`)

## `memory.metadata`

| column | type | nullable | default |
|---|---|---|---|
| `key` | text | no |  |
| `value` | text | no |  |
| `updated_at` | timestamp with time zone | no | `now()` |

- PRIMARY KEY `key` (`metadata_pkey`)

## `runtime.search_documents`

| column | type | nullable | default |
|---|---|---|---|
| `search_document_id` | bigint | no | `nextval('runtime.search_documents_search_document_id_seq'::regclass)` |
| `owner_table` | text | no |  |
| `owner_key` | text | no |  |
| `language` | regconfig | no | `'english'::regconfig` |
| `search_vector` | tsvector | no |  |
| `updated_at` | timestamp with time zone | no | `now()` |
| `search_text` | text | no |  |

- UNIQUE `owner_table` (`search_documents_owner_table_owner_key_key`)
- UNIQUE `owner_key` (`search_documents_owner_table_owner_key_key`)
- PRIMARY KEY `search_document_id` (`search_documents_pkey`)

## `runtime.workflow_agent_calls`

| column | type | nullable | default |
|---|---|---|---|
| `call_id` | text | no |  |
| `run_id` | text | no |  |
| `phase_run_id` | text | yes |  |
| `replay_key` | text | no |  |
| `cache_key` | text | no |  |
| `call_ordinal` | integer | no |  |
| `prompt` | text | no |  |
| `options_json` | jsonb | no | `'{}'::jsonb` |
| `child_agent_id` | text | yes |  |
| `status` | text | no | `'pending'::text` |
| `cached_from_call_id` | text | yes |  |
| `final_response` | text | yes |  |
| `input_tokens` | bigint | no | `0` |
| `output_tokens` | bigint | no | `0` |
| `tool_call_count` | integer | no | `0` |
| `duration_ms` | bigint | yes |  |
| `created_at` | timestamp with time zone | no | `now()` |
| `started_at` | timestamp with time zone | yes |  |
| `updated_at` | timestamp with time zone | no | `now()` |
| `completed_at` | timestamp with time zone | yes |  |
| `error` | text | yes |  |

- FOREIGN KEY `cached_from_call_id` → `runtime.workflow_agent_calls.call_id`
- FOREIGN KEY `phase_run_id` → `runtime.workflow_phase_runs.phase_run_id`
- PRIMARY KEY `call_id` (`workflow_agent_calls_pkey`)
- FOREIGN KEY `run_id` → `runtime.workflow_runs.run_id`
- UNIQUE `run_id` (`workflow_agent_calls_run_id_replay_key_key`)
- UNIQUE `replay_key` (`workflow_agent_calls_run_id_replay_key_key`)

## `runtime.workflow_phase_runs`

| column | type | nullable | default |
|---|---|---|---|
| `phase_run_id` | text | no |  |
| `run_id` | text | no |  |
| `phase_name` | text | no |  |
| `ordinal` | integer | no |  |
| `status` | text | no | `'pending'::text` |
| `agent_total` | integer | no | `0` |
| `agent_completed` | integer | no | `0` |
| `input_tokens` | bigint | no | `0` |
| `output_tokens` | bigint | no | `0` |
| `started_at` | timestamp with time zone | yes |  |
| `updated_at` | timestamp with time zone | no | `now()` |
| `completed_at` | timestamp with time zone | yes |  |
| `error` | text | yes |  |

- PRIMARY KEY `phase_run_id` (`workflow_phase_runs_pkey`)
- FOREIGN KEY `run_id` → `runtime.workflow_runs.run_id`
- UNIQUE `run_id` (`workflow_phase_runs_unique_phase`)
- UNIQUE `phase_name` (`workflow_phase_runs_unique_phase`)

## `runtime.workflow_runs`

| column | type | nullable | default |
|---|---|---|---|
| `run_id` | text | no |  |
| `launch_mode` | text | no | `'async'::text` |
| `resume_from_run_id` | text | yes |  |
| `workflow_name` | text | no |  |
| `description` | text | no | `''::text` |
| `phases_json` | jsonb | no | `'[]'::jsonb` |
| `args_json` | jsonb | no | `'{}'::jsonb` |
| `workspace_root` | text | no |  |
| `script_path` | text | no |  |
| `script_sha256` | text | no |  |
| `recovery_count` | integer | no | `0` |
| `last_recovery_reason` | text | yes |  |
| `last_recovered_at` | timestamp with time zone | yes |  |
| `status` | text | no | `'queued'::text` |
| `error` | text | yes |  |
| `final_result` | text | yes |  |
| `created_at` | timestamp with time zone | no | `now()` |
| `started_at` | timestamp with time zone | yes |  |
| `updated_at` | timestamp with time zone | no | `now()` |
| `completed_at` | timestamp with time zone | yes |  |
| `terminal_handoff_message_id` | text | yes |  |
| `terminal_handoff_delivered_at` | timestamp with time zone | yes |  |
| `terminal_handoff_disposition` | text | yes |  |
| `terminal_handoff_generation` | integer | no | `0` |

- PRIMARY KEY `run_id` (`workflow_runs_pkey`)
- FOREIGN KEY `resume_from_run_id` → `runtime.workflow_runs.run_id`

## `scheduler.cron_mutations`

| column | type | nullable | default |
|---|---|---|---|
| `mutation_seq` | bigint | no | `nextval('scheduler.cron_mutations_mutation_seq_seq'::regclass)` |
| `job_id` | text | no |  |
| `action` | text | no |  |
| `occurred_at_ms` | bigint | no |  |
| `carrier_message_id` | text | no |  |
| `request_id` | text | no |  |
| `tool_call_id` | text | no |  |
| `input_message_ids` | text[] | no |  |
| `created_at` | timestamp with time zone | no | `now()` |

- PRIMARY KEY `mutation_seq` (`cron_mutations_pkey`)

## `scheduler.delivery_outbox`

| column | type | nullable | default |
|---|---|---|---|
| `delivery_key` | text | no |  |
| `job_id` | text | no |  |
| `run_id` | text | no |  |
| `payload_json` | text | no |  |
| `state` | text | no |  |
| `attempt_count` | integer | no | `0` |
| `next_attempt_at_utc` | bigint | no |  |
| `last_error` | text | yes |  |
| `created_at_utc` | bigint | no |  |
| `updated_at_utc` | bigint | no |  |
| `delivered_at_utc` | bigint | yes |  |

- PRIMARY KEY `delivery_key` (`delivery_outbox_pkey`)

## `scheduler.job_definitions`

| column | type | nullable | default |
|---|---|---|---|
| `job_definition_id` | bigint | no | `nextval('scheduler.job_definitions_job_definition_id_seq'::regclass)` |
| `job_id` | text | no |  |
| `version` | integer | no |  |
| `prompt_template_id` | bigint | yes |  |
| `created_at` | timestamp with time zone | no | `now()` |
| `retired_at` | timestamp with time zone | yes |  |
| `task_text` | text | no |  |

- PRIMARY KEY `job_definition_id` (`job_definitions_pkey`)

## `scheduler.job_runs`

| column | type | nullable | default |
|---|---|---|---|
| `run_id` | text | no |  |
| `job_id` | text | no |  |
| `job_definition_id` | bigint | yes |  |
| `request_id` | text | yes |  |
| `scheduled_for` | timestamp with time zone | no |  |
| `scheduled_for_utc` | bigint | no |  |
| `trigger_reason` | text | yes |  |
| `started_at` | timestamp with time zone | yes |  |
| `started_at_utc` | bigint | yes |  |
| `finished_at` | timestamp with time zone | yes |  |
| `finished_at_utc` | bigint | yes |  |
| `status` | scheduler.run_status | no |  |
| `result_summary` | text | yes |  |
| `error_text` | text | yes |  |
| `attempt` | integer | no | `1` |
| `worker_phase` | text | yes |  |
| `worker_agent_id` | text | yes |  |
| `worker_execution_timed_out` | boolean | no | `false` |
| `notification_policy_json` | text | yes |  |

- PRIMARY KEY `run_id` (`job_runs_pkey`)

## `scheduler.jobs`

| column | type | nullable | default |
|---|---|---|---|
| `job_id` | text | no |  |
| `is_heartbeat` | boolean | no | `false` |
| `source_path` | text | yes |  |
| `source_hash` | text | yes |  |
| `schedule_kind` | text | no |  |
| `enabled` | boolean | no | `true` |
| `schedule_expr` | text | no |  |
| `timezone` | text | no | `'UTC'::text` |
| `retry_on_failure` | boolean | no | `false` |
| `max_retries` | integer | no | `0` |
| `delivery_targets_json` | text | no | `'[]'::text` |
| `next_run_at` | timestamp with time zone | yes |  |
| `next_run_at_utc` | bigint | no | `0` |
| `last_run_at_utc` | bigint | yes |  |
| `last_success_at_utc` | bigint | yes |  |
| `last_status` | text | yes |  |
| `consecutive_failures` | bigint | no | `0` |
| `updated_at_utc` | bigint | no | `0` |
| `created_at` | timestamp with time zone | no | `now()` |
| `updated_at` | timestamp with time zone | no | `now()` |

- PRIMARY KEY `job_id` (`jobs_pkey`)

## `scheduler.terminal_signals`

| column | type | nullable | default |
|---|---|---|---|
| `run_id` | text | no |  |
| `signal_type` | text | no |  |
| `message` | text | yes |  |
| `producer_request_trace_json` | text | yes |  |
| `created_at` | timestamp with time zone | no | `now()` |
| `created_at_utc` | bigint | no | `0` |
| `producer_agent_id` | text | yes |  |
| `producer_phase_attempt` | integer | yes |  |
| `blocked_reason` | text | yes |  |
| `blocked_dependency` | text | yes |  |

- PRIMARY KEY `run_id` (`terminal_signals_pkey`)
