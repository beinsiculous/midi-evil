-- Original Maxwell control glyphs. Each editable layer occupies a 16px atlas cell.
local sprite = Sprite(128, 16, ColorMode.RGB)
sprite.layers[1].name = 'transparent base'
local cream = app.pixelColor.rgba(249,226,159,255)
local red = app.pixelColor.rgba(216,73,91,255)
local function glyph(index, name, draw)
  local layer = sprite:newLayer()
  layer.name = name
  local image = Image(128,16,ColorMode.RGB)
  local function box(x,y,w,h,color)
    for py=y,y+h-1 do for px=x,x+w-1 do image:drawPixel(index*16+px,py,color or cream) end end
  end
  draw(box)
  sprite:newCel(layer,1,image,Point(0,0))
end
glyph(0,'01 song',function(b) b(2,3,11,2); b(2,7,5,2); b(8,7,5,2,red); b(2,11,8,2) end)
glyph(1,'02 piano roll',function(b) b(2,3,2,10); b(6,3,2,10); b(10,3,2,10); b(4,3,1,6,red); b(8,3,1,6,red) end)
glyph(2,'03 mixer',function(b) for x=3,11,4 do b(x,2,1,12) end; b(1,5,5,3,red); b(5,9,5,3); b(9,4,5,3) end)
glyph(3,'04 synth',function(b) b(1,8,3,2); b(4,4,2,6); b(6,3,2,2,red); b(8,5,2,6); b(10,10,2,2,red); b(12,7,3,2) end)
glyph(4,'05 play',function(b) for x=4,11 do local d=math.min(x-4,11-x); local h=math.floor((11-x)*0.7); b(x,7-h,1,h*2+1) end end)
glyph(5,'06 stop',function(b) b(4,4,8,8,red) end)
glyph(6,'07 save',function(b) b(2,2,11,12); b(4,3,6,4,red); b(4,9,7,4,app.pixelColor.rgba(43,36,53,255)); b(6,10,3,2) end)
glyph(7,'08 project',function(b) b(2,3,5,2,red); b(2,5,12,8); b(3,6,10,6,app.pixelColor.rgba(43,36,53,255)) end)
sprite:saveAs('crates/midi-evil/assets/maxwell-controls.aseprite')
sprite:saveCopyAs('crates/midi-evil/assets/maxwell-controls.png')
