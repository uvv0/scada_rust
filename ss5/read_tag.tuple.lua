local port = 2
local device = 1
local id = 1
local value, valid = tag.get(port, device, id)

if value == nil then
    log.write("port 2 device 1 tag 1 is not registered")
elseif valid then
    log.write("lua54 ok, port 2 device 1 tag 1=" .. tostring(value))
else
    log.write("lua54 ok, port 2 device 1 tag 1 is invalid")
end
