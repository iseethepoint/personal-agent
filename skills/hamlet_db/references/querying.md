# Querying with hamlet.db

- One PostgreSQL `SELECT` per call; tables are schema-qualified (`memory.entries`), as listed in `schema.md`.
- CTE names start with `hatch_cte_`; recursive CTEs, `SELECT INTO`, locking clauses, table functions, `LATERAL` and sampling are rejected.
- Give every selected column a unique name; duplicate output names are rejected.
- At most 200 rows come back; a row larger than 65536 bytes of JSON is replaced by `{"_hatch_db_row_omitted": true, "serialized_bytes": N}` — select fewer or shorter columns.
- The query runs read-only with a 3000 ms statement timeout and a 5000 ms total limit.

## Functions

Only these built-in functions are accepted:

`abs`, `age`, `array_length`, `array_position`, `array_to_string`, `avg`, `bit_and`, `bit_or`, `bool_and`, `bool_or`, `btrim`, `cardinality`, `ceil`, `ceiling`, `char_length`, `coalesce`, `concat`, `concat_ws`, `count`, `date_bin`, `date_part`, `date_trunc`, `dense_rank`, `encode`, `every`, `extract`, `first_value`, `floor`, `greatest`, `json_array_length`, `json_build_array`, `json_build_object`, `json_extract_path`, `json_extract_path_text`, `json_object_keys`, `json_typeof`, `jsonb_array_length`, `jsonb_build_array`, `jsonb_build_object`, `jsonb_extract_path`, `jsonb_extract_path_text`, `jsonb_object_keys`, `jsonb_typeof`, `lag`, `last_value`, `lead`, `least`, `left`, `length`, `lower`, `ltrim`, `make_interval`, `max`, `md5`, `min`, `mod`, `now`, `nth_value`, `ntile`, `nullif`, `octet_length`, `position`, `power`, `rank`, `regexp_match`, `regexp_replace`, `replace`, `reverse`, `right`, `round`, `row_number`, `rtrim`, `split_part`, `sqrt`, `strpos`, `substr`, `substring`, `sum`, `time_bucket`, `timezone`, `to_char`, `to_json`, `to_jsonb`, `trim`, `upper`

## Casts

Only these cast types are accepted (with `::type` or `CAST(… AS type)`):

`bigint`, `bool`, `boolean`, `bytea`, `char`, `character`, `character varying`, `date`, `decimal`, `double precision`, `float4`, `float8`, `int`, `int2`, `int4`, `int8`, `integer`, `interval`, `json`, `jsonb`, `numeric`, `real`, `smallint`, `text`, `time`, `timestamp`, `timestamp with time zone`, `timestamp without time zone`, `timestamptz`, `timetz`, `uuid`, `varchar`
