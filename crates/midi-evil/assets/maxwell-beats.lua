-- Original Maxwell rhythm-lab badge. Run in Aseprite from the repository root.
local sprite=Sprite(160,96,ColorMode.RGB)
sprite.layers[1].name='rhythm circuitry'
local image=Image(160,96,ColorMode.RGB)
local C={bg=app.pixelColor.rgba(30,26,38,255),red=app.pixelColor.rgba(216,73,91,255),cream=app.pixelColor.rgba(249,226,159,255),purple=app.pixelColor.rgba(166,128,208,255),edge=app.pixelColor.rgba(68,57,79,255)}
local function box(x,y,w,h,color)
  for py=y,y+h-1 do for px=x,x+w-1 do image:drawPixel(px,py,color) end end
end
box(0,0,160,96,C.bg)
for _,origin in ipairs{8,120} do
  for row=0,3 do for col=0,3 do box(origin+col*8,16+row*8,6,6,(row+col)%3==0 and C.red or C.edge) end end
end
box(7,64,146,1,C.edge)
local letters={E={'111','100','110','100','111'},V={'101','101','101','101','010'},I={'111','010','010','010','111'},L={'100','100','100','100','111'},B={'110','101','110','101','110'},A={'010','101','111','101','101'},T={'111','010','010','010','010'},S={'111','100','111','001','111'}}
local x=21
for char in ('EVIL BEATS'):gmatch('.') do
  local rows=letters[char]
  if rows then for y,row in ipairs(rows) do for col=1,3 do if row:sub(col,col)=='1' then box(x+(col-1)*2,74+(y-1)*2,2,2,C.cream) end end end end
  x=x+12
end
sprite:newCel(sprite.layers[1],1,image,Point(0,0))
local icon=app.open('crates/midi-evil/assets/midi-evil-icon.aseprite')
local flat=Image(icon.width,icon.height,ColorMode.RGB); flat:drawSprite(icon,1)
app.activeSprite=sprite
local layer=sprite:newLayer(); layer.name='Dr. Maxwell / selected original branding'
sprite:newCel(layer,1,flat,Point(48,0))
sprite:saveAs('crates/midi-evil/assets/maxwell-beats.aseprite')
sprite:saveCopyAs('crates/midi-evil/assets/maxwell-beats.png')
