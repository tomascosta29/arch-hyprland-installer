hl.monitor({
    output = "DP-1",
    mode = "2560x1440@180",
    position = "0x0",
    scale = 1,
    vrr = 1,
})
hl.monitor({
    output = "HDMI-A-1",
    mode = "2560x1440@144.01",
    position = "2560x0",
    scale = 1,
})

for workspace = 1, 4 do
    hl.workspace_rule({
        workspace = tostring(workspace),
        monitor = "HDMI-A-1",
        default = workspace == 1,
        persistent = true,
    })
end

for workspace = 5, 8 do
    hl.workspace_rule({
        workspace = tostring(workspace),
        monitor = "DP-1",
        default = workspace == 5,
        persistent = true,
    })
end

for workspace = 9, 10 do
    hl.workspace_rule({
        workspace = tostring(workspace),
        monitor = "HDMI-A-1",
        default = workspace == 9,
        persistent = true,
    })
end
