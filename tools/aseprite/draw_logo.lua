-- Draw Rusty Music's original 32x32 studio logo.
local output = app.params["output"]
assert(output, "pass --script-param output=/path/to/rusty-logo.aseprite")

local sprite = Sprite(32, 32, ColorMode.RGBA)
local image = sprite.cels[1].image
local ink = Color { r = 16, g = 24, b = 32, a = 255 }
local teal = Color { r = 54, g = 214, b = 195, a = 255 }
local orange = Color { r = 255, g = 170, b = 76, a = 255 }
local cream = Color { r = 242, g = 242, b = 233, a = 255 }
local clear = Color { r = 0, g = 0, b = 0, a = 0 }

local function pixel(x, y, color)
  if x >= 0 and x < 32 and y >= 0 and y < 32 then
    image:putPixel(x, y, color)
  end
end

for y = 0, 31 do
  for x = 0, 31 do
    pixel(x, y, clear)
  end
end

-- Rounded square with a teal pixel border.
for y = 1, 30 do
  for x = 1, 30 do
    local corner = (x < 5 and y < 5 and x + y < 7)
      or (x > 26 and y < 5 and (31 - x) + y < 7)
      or (x < 5 and y > 26 and x + (31 - y) < 7)
      or (x > 26 and y > 26 and (31 - x) + (31 - y) < 7)
    if not corner then
      local border = x < 3 or x > 28 or y < 3 or y > 28
      pixel(x, y, border and teal or ink)
    end
  end
end

-- Five chunky waveform bars, mirrored around the center line.
local heights = { 6, 12, 20, 14, 8 }
for index, height in ipairs(heights) do
  local center_x = 5 + index * 4
  local top = math.floor(16 - height / 2)
  for x = center_x, center_x + 2 do
    for y = top, top + height - 1 do
      pixel(x, y, index == 3 and cream or orange)
    end
  end
end

sprite:saveAs(output)
