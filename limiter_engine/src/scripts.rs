pub const SLIDING_WINDOW_SCRIPT: &str = r#"
local current_key = KEYS[1]
local previous_key = KEYS[2]

local max_requests = tonumber(ARGV[1])
local window_size_ms = tonumber(ARGV[2])
local elapsed_ms = tonumber(ARGV[3])

local previous_count = tonumber(redis.call("GET", previous_key) or "0")
local current_count = tonumber(redis.call("GET", current_key) or "0")

local weight = (window_size_ms - elapsed_ms) / window_size_ms
if weight < 0 then weight = 0 end

local estimated_count = current_count + (previous_count * weight)

if estimated_count >= max_requests then
    return {0, tostring(estimated_count), current_count, previous_count}
end

current_count = redis.call("INCR", current_key)

if current_count == 1 then
    redis.call("PEXPIRE", current_key, window_size_ms * 2)
end

estimated_count = current_count + (previous_count * weight)
return {1, tostring(estimated_count), current_count, previous_count}
"#;
