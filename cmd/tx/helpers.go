package tx

// getString extracts a string from a map by traversing nested keys.
// Example: getString(m, "data", "metadata", "inner") returns m["data"]["metadata"]["inner"]
func getString(m map[string]any, keys ...string) string {
	if m == nil || len(keys) == 0 {
		return ""
	}

	current := m
	for _, key := range keys[:len(keys)-1] {
		nested, ok := current[key].(map[string]any)
		if !ok {
			return ""
		}
		current = nested
	}

	if v, ok := current[keys[len(keys)-1]].(string); ok {
		return v
	}
	return ""
}
