local _M = {}

local _extract_default

local function filter_type(schema)
    if schema == nil then
        return nil
    end
    local count = 0
    for _ in pairs(schema) do
        count = count + 1
    end
    if count == 1 and schema.type ~= nil then
        return nil
    elseif count == 1 and schema.properties ~= nil and next(schema.properties) == nil then
        return nil
    elseif count == 2 and schema.type == "object" and schema.properties ~= nil and next(schema.properties) == nil then
        return nil
    elseif count == 2 and schema.type ~= nil and schema.default ~= nil then
        return schema.default
    elseif count == 1 and schema.type == nil and schema.default ~= nil then
        return schema.default
    elseif count == 1 and schema.type == nil and schema.properties ~= nil then
        return schema.properties
    end
    return schema
end

local function extract_default(schema)
    if schema == nil then
        return schema
    end
    schema = _extract_default(schema)
    if schema == nil then
        return schema
    end
    if next(schema) == nil then
        return nil
    end

    schema = filter_type(schema)
    return schema
end

-- non-recursive equal, array order is not considered
local function table_equal(o1, o2)
    if o1 == o2 then return true end
    local o1Type = type(o1)
    local o2Type = type(o2)
    if o1Type ~= o2Type then return false end

    if o1Type == "table" then
        local keySet = {}

        for key1, value1 in pairs(o1) do
            local value2 = o2[key1]
            if value2 == nil or table_equal(value1, value2, ignore_mt) == false then
                return false
            end
            keySet[key1] = true
        end

        for key2, _ in pairs(o2) do
            if not keySet[key2] then return false end
        end
        return true
    end

    return false
end

_extract_default = function (schema)
    schema["description"] = nil
    schema["$comment"] = nil
    schema["enum"] = nil
    schema["required"] = nil
    schema["minItems"] = nil
    schema["maxItems"] = nil
    schema["uniqueItems"] = nil
    schema["minLength"] = nil
    schema["maxLength"] = nil
    schema["minimum"] = nil
    schema["maximum"] = nil
    schema["pattern"] = nil
    schema["encrypt_fields"] = nil
    schema["minProperties"] = nil
    --schema["additionalProperties"] = nil -- keep this
    schema["title"] = nil

    if schema.allOf then
        for i, def in ipairs(schema.allOf) do
            schema.allOf[i] = extract_default(def)
        end
        if #schema.allOf == 0 then
            schema.allOf = nil
        end
    end
    if schema.oneOf then
        for i, def in ipairs(schema.oneOf) do
            schema.oneOf[i] = extract_default(def)
        end
        if #schema.oneOf == 0 then
            schema.oneOf = nil
        end
    end
    if schema.anyOf then
        for i, def in ipairs(schema.anyOf) do
            schema.anyOf[i] = extract_default(def)
        end
        if #schema.anyOf == 0 then
            schema.anyOf = nil
        end
    end

    if schema["if"] ~= nil then
        schema["then"] = extract_default(schema["then"])
        schema["else"] = extract_default(schema["else"])
    end

    --if not schema.type and not schema.default then
    --    -- Not a valid schema, return as is
    --    return schema
    --end

    if schema.type == "object" or (schema.type == nil and schema.properties ~= nil) then
        -- Object schema, iterate properties
        if not schema.properties and not schema.patternProperties then
            -- no properties? return it as is
            if not schema.default then
                return nil
            end
            return schema
        end
        if schema.properties ~= nil then
            for prop, prop_schema in pairs(schema.properties) do
                schema.properties[prop] = extract_default(prop_schema)
            end
            if schema.properties ~= nil and next(schema.properties) == nil then
                schema.properties = nil
            end

            if schema.properties ~= nil and schema.default ~= nil then
                if table_equal(schema.properties, schema.default) then
                    schema.default = nil
                end
            end
            if schema.properties ~= nil and schema.default == nil then
                for prop, prop_schema in pairs(schema.properties) do
                    schema[prop] = prop_schema
                end
                schema.properties = nil
            end
        end

        if schema.patternProperties ~= nil then
            for prop, prop_schema in pairs(schema.patternProperties) do
                schema.patternProperties[prop] = extract_default(prop_schema)
            end
            if schema.patternProperties ~= nil and next(schema.patternProperties) == nil then
                schema.patternProperties = nil
            end
        end

        schema.type = nil
    elseif schema.type == "array" then
        if not schema.items then
            return schema
        end

        schema.items = extract_default(schema.items)
    elseif schema.type == "string" or schema.type == "integer" or schema.type == "number" or schema.type == "boolean" then
        if schema.default == nil then
            -- No default value, ignore
            return nil
        end
    else
        return schema
    end

    return schema
end

function _M.get_plugins_schema_list(subsystem)
    local plugins = {}

    for dir in io.popen([[ls -pa ./apisix/plugins | grep -v /]]):lines() do
        if string.sub(dir, -4) == ".lua" then
            print(dir)
            print(string.sub(dir, 1, #dir-4))
            local file = string.sub(dir, 1, #dir-4)
            local plugin = require("apisix.plugins."..file)
            plugins[file] = plugin.schema
        end
    end

    return plugins
end

function _M.get_plugins_filtered_schema_list(subsystem)
    local plugins = {}

    for dir in io.popen([[ls -pa ./apisix/plugins | grep -v /]]):lines() do
        if string.sub(dir, -4) == ".lua" then
            print(dir)
            print(string.sub(dir, 1, #dir-4))
            local file = string.sub(dir, 1, #dir-4)
            local plugin = require("apisix.plugins."..file)

            -- manual patch
            if file == "openfunction" then
                plugin.schema.properties.authorization = nil
            end

            plugins[file] = extract_default(plugin.schema)

            -- manual patch
            if file == "opentelemetry" then
                plugins[file].sampler = plugins[file].sampler.default
            end
        end
    end

    return plugins
end

return _M
