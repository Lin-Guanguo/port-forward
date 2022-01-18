config = {
    port = 8077;
    users = {
        {
            uuid = "936DA01F9ABD4d9d80C702AF85C822A8",
            ports = {"8080:8001", "8088:8002"}
        }
    }
}

function Dump(o)
    if type(o) == 'table' then
        local s = '{ '
        for k,v in pairs(o) do
            if type(k) ~= 'number' then k = '"'..k..'"' end
            s = s .. '['..k..'] = ' .. Dump(v) .. ','
        end
        return s .. '} '
    else
        return tostring(o)
    end
end

-- print(config, Dump(config))