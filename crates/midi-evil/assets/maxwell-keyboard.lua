-- Original Maxwell keyboard badge; run from the repository root in Aseprite.
local sprite = Sprite(160,96,ColorMode.RGB)
sprite.layers[1].name='panel circuitry'
local image=Image(160,96,ColorMode.RGB)
local C={bg=Color{r=30,g=26,b=38},edge=Color{r=68,g=57,b=79},red=Color{r=216,g=73,b=91},cream=Color{r=249,g=226,b=159},dark=Color{r=19,g=17,b=25}}
local function box(x,y,w,h,color)
  for py=y,y+h-1 do for px=x,x+w-1 do image:drawPixel(px,py,color.rgbaPixel) end end
end
box(0,0,160,96,C.bg)
for _,xx in ipairs{8,151} do box(xx,8,2,2,C.cream); box(xx,85,2,2,C.cream) end
for i=0,6 do
  local h=5+((i*7)%19)
  box(18+i*4,52-h,2,h,C.edge); box(18+i*4,51,2,2,C.red)
  box(116+i*4,52-h,2,h,C.edge); box(116+i*4,51,2,2,C.red)
end
box(12,66,136,1,C.edge); box(12,64,2,3,C.red); box(146,64,2,3,C.red)
local letters={E={'111','100','110','100','111'},V={'101','101','101','101','010'},I={'111','010','010','010','111'},L={'100','100','100','100','111'},K={'101','101','110','101','101'},Y={'101','101','010','010','010'},S={'111','100','111','001','111'}}
local x=30
for char in ('EVIL KEYS'):gmatch('.') do
  local rows=letters[char]
  if rows then for y,row in ipairs(rows) do for col=1,3 do if row:sub(col,col)=='1' then box(x+(col-1)*2,74+(y-1)*2,2,2,C.cream) end end end end
  x=x+12
end
sprite:newCel(sprite.layers[1],1,image,Point(0,0))
local icon=app.open('crates/midi-evil/assets/midi-evil-icon.aseprite')
local flat=Image(icon.width,icon.height,ColorMode.RGB)
flat:drawSprite(icon,1)
app.activeSprite=sprite
local layer=sprite:newLayer(); layer.name='Dr. Maxwell / original selected branding'
sprite:newCel(layer,1,flat,Point(48,0))
sprite:saveAs('crates/midi-evil/assets/maxwell-keyboard.aseprite')
sprite:saveCopyAs('crates/midi-evil/assets/maxwell-keyboard.png')
-- Add the keyboard tab glyph to the unused last atlas cell; keep all existing cells.
local controls=app.open('crates/midi-evil/assets/maxwell-controls.aseprite')
local old
for _,l in ipairs(controls.layers) do if l.name=='08 project' or l.name=='08 keyboard' then old=l end end
if old then controls:deleteLayer(old) end
local layer=controls:newLayer(); layer.name='08 keyboard'
local keys=Image(128,16,ColorMode.RGB)
for x=114,125 do for y=3,12 do keys:drawPixel(x,y,C.cream.rgbaPixel) end end
for _,x in ipairs{117,121} do for y=3,8 do keys:drawPixel(x,y,C.red.rgbaPixel) end end
for _,x in ipairs{117,121} do for y=9,12 do keys:drawPixel(x,y,C.dark.rgbaPixel) end end
controls:newCel(layer,1,keys,Point(0,0))
controls:saveAs('crates/midi-evil/assets/maxwell-controls.aseprite')
controls:saveCopyAs('crates/midi-evil/assets/maxwell-controls.png')
