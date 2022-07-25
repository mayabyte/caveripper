### ALL CODE HERE FROM https://github.com/LagoLunatic/GCFT
### Relevant code for extracting BTI images was copied here to be used from scripts
### rather than through a GUI.

from io import BytesIO
from enum import Enum
from PIL import Image
import struct
import sys

class WrapMode(Enum):
  ClampToEdge    = 0
  Repeat         = 1
  MirroredRepeat = 2

class FilterMode(Enum):
  Nearest              = 0
  Linear               = 1
  NearestMipmapNearest = 2
  NearestMipmapLinear  = 3
  LinearMipmapNearest  = 4
  LinearMipmapLinear   = 5

class BTI:
  def __init__(self, data, header_offset=0):
    self.data = data
    self.header_offset = header_offset
    
    self.read_header(data, header_offset=header_offset)
    
    blocks_wide = (self.width + (self.block_width-1)) // self.block_width
    blocks_tall = (self.height + (self.block_height-1)) // self.block_height
    image_data_size = blocks_wide*blocks_tall*self.block_data_size
    remaining_mipmaps = self.mipmap_count-1
    curr_mipmap_size = image_data_size
    while remaining_mipmaps > 0:
      # Each mipmap is a quarter the size of the last (half the width and half the height).
      curr_mipmap_size = curr_mipmap_size//4
      image_data_size += curr_mipmap_size
      remaining_mipmaps -= 1
      # Note: We don't actually read the smaller mipmaps, we only read the normal sized one, and when saving recalculate the others by scaling the normal one down.
      # This is to simplify things, but a full implementation would allow reading and saving each mipmap individually (since the mipmaps can actually have different contents).
    self.image_data = BytesIO(read_bytes(data, header_offset+self.image_data_offset, image_data_size))
    
    palette_data_size = self.num_colors*2
    self.palette_data = BytesIO(read_bytes(data, header_offset+self.palette_data_offset, palette_data_size))
  
  def read_header(self, data, header_offset=0):
    self.image_format = ImageFormat(read_u8(data, header_offset+0))
    
    self.alpha_setting = read_u8(data, header_offset+1)
    self.width = read_u16(data, header_offset+2)
    self.height = read_u16(data, header_offset+4)
    
    self.wrap_s = WrapMode(read_u8(data, header_offset+6))
    self.wrap_t = WrapMode(read_u8(data, header_offset+7))
    
    self.palettes_enabled = bool(read_u8(data, header_offset+8))
    self.palette_format = PaletteFormat(read_u8(data, header_offset+9))
    self.num_colors = read_u16(data, header_offset+0xA)
    self.palette_data_offset = read_u32(data, header_offset+0xC)
    
    self.min_filter = FilterMode(read_u8(data, header_offset+0x14))
    self.mag_filter = FilterMode(read_u8(data, header_offset+0x15))
    
    self.min_lod = read_u8(data, header_offset+0x16)
    self.max_lod = read_u8(data, header_offset+0x17) # seems to be equal to (mipmap_count-1)*8
    self.mipmap_count = read_u8(data, header_offset+0x18)
    self.unknown_3 = read_u8(data, header_offset+0x19)
    self.lod_bias = read_u16(data, header_offset+0x1A)
    
    self.image_data_offset = read_u32(data, header_offset+0x1C)
  
  @property
  def block_width(self):
    return BLOCK_WIDTHS[self.image_format]
  
  @property
  def block_height(self):
    return BLOCK_HEIGHTS[self.image_format]
  
  @property
  def block_data_size(self):
    return BLOCK_DATA_SIZES[self.image_format]
  
  def render(self):
    image = decode_image(
      self.image_data, self.palette_data,
      self.image_format, self.palette_format,
      self.num_colors,
      self.width, self.height
    )
    return image

class BTIFile(BTI): # For standalone .bti files (as opposed to textures embedded inside J3D models/animations)
  def __init__(self, data):
    if Yaz0.check_is_compressed(data):
      data = Yaz0.decompress(data)
    super(BTIFile, self).__init__(data)

class TooManyColorsError(Exception):
  pass

class ImageFormat(Enum):
  I4     =   0
  I8     =   1
  IA4    =   2
  IA8    =   3
  RGB565 =   4
  RGB5A3 =   5
  RGBA32 =   6
  C4     =   8
  C8     =   9
  C14X2  = 0xA
  CMPR   = 0xE

class PaletteFormat(Enum):
  IA8    = 0
  RGB565 = 1
  RGB5A3 = 2

BLOCK_WIDTHS = {
  ImageFormat.I4    : 8,
  ImageFormat.I8    : 8,
  ImageFormat.IA4   : 8,
  ImageFormat.IA8   : 4,
  ImageFormat.RGB565: 4,
  ImageFormat.RGB5A3: 4,
  ImageFormat.RGBA32: 4,
  ImageFormat.C4    : 8,
  ImageFormat.C8    : 8,
  ImageFormat.C14X2 : 4,
  ImageFormat.CMPR  : 8,
}
BLOCK_HEIGHTS = {
  ImageFormat.I4    : 8,
  ImageFormat.I8    : 4,
  ImageFormat.IA4   : 4,
  ImageFormat.IA8   : 4,
  ImageFormat.RGB565: 4,
  ImageFormat.RGB5A3: 4,
  ImageFormat.RGBA32: 4,
  ImageFormat.C4    : 8,
  ImageFormat.C8    : 4,
  ImageFormat.C14X2 : 4,
  ImageFormat.CMPR  : 8,
}
BLOCK_DATA_SIZES = {
  ImageFormat.I4    : 32,
  ImageFormat.I8    : 32,
  ImageFormat.IA4   : 32,
  ImageFormat.IA8   : 32,
  ImageFormat.RGB565: 32,
  ImageFormat.RGB5A3: 32,
  ImageFormat.RGBA32: 64,
  ImageFormat.C4    : 32,
  ImageFormat.C8    : 32,
  ImageFormat.C14X2 : 32,
  ImageFormat.CMPR  : 32,
}

IMAGE_FORMATS_THAT_USE_PALETTES = [
  ImageFormat.C4,
  ImageFormat.C8,
  ImageFormat.C14X2,
]

def swizzle_3_bit_to_8_bit(v):
  # 00000123 -> 12312312
  return (v << 5) | (v << 2) | (v >> 1)

def swizzle_4_bit_to_8_bit(v):
  # 00001234 -> 12341234
  return (v << 4) | (v >> 0)

def swizzle_5_bit_to_8_bit(v):
  # 00012345 -> 12345123
  return (v << 3) | (v >> 2)

def swizzle_6_bit_to_8_bit(v):
  # 00123456 -> 12345612
  return (v << 2) | (v >> 4)

def convert_rgb565_to_color(rgb565):
  r = ((rgb565 >> 11) & 0x1F)
  g = ((rgb565 >> 5) & 0x3F)
  b = ((rgb565 >> 0) & 0x1F)
  r = swizzle_5_bit_to_8_bit(r)
  g = swizzle_6_bit_to_8_bit(g)
  b = swizzle_5_bit_to_8_bit(b)
  return (r, g, b, 255)

def convert_rgb5a3_to_color(rgb5a3):
  # RGB5A3 format.
  # Each color takes up two bytes.
  # Format depends on the most significant bit. Two possible formats:
  # Top bit is 0: 0AAARRRRGGGGBBBB
  # Top bit is 1: 1RRRRRGGGGGBBBBB (Alpha set to 0xff)
  if (rgb5a3 & 0x8000) == 0:
    a = ((rgb5a3 >> 12) & 0x7)
    r = ((rgb5a3 >> 8) & 0xF)
    g = ((rgb5a3 >> 4) & 0xF)
    b = ((rgb5a3 >> 0) & 0xF)
    a = swizzle_3_bit_to_8_bit(a)
    r = swizzle_4_bit_to_8_bit(r)
    g = swizzle_4_bit_to_8_bit(g)
    b = swizzle_4_bit_to_8_bit(b)
  else:
    a = 255
    r = ((rgb5a3 >> 10) & 0x1F)
    g = ((rgb5a3 >> 5) & 0x1F)
    b = ((rgb5a3 >> 0) & 0x1F)
    r = swizzle_5_bit_to_8_bit(r)
    g = swizzle_5_bit_to_8_bit(g)
    b = swizzle_5_bit_to_8_bit(b)
  return (r, g, b, a)

def convert_ia4_to_color(ia4):
  low_nibble = ia4 & 0xF
  high_nibble = (ia4 >> 4) & 0xF
  
  r = g = b = swizzle_4_bit_to_8_bit(low_nibble)
  a = swizzle_4_bit_to_8_bit(high_nibble)
  
  return (r, g, b, a)

def convert_ia8_to_color(ia8):
  low_byte = ia8 & 0xFF
  high_byte = (ia8 >> 8) & 0xFF
  
  r = g = b = low_byte
  a = high_byte
  
  return (r, g, b, a)

def convert_i4_to_color(i4):
  r = g = b = a = swizzle_4_bit_to_8_bit(i4)
  
  return (r, g, b, a)

def convert_i8_to_color(i8):
  r = g = b = a = i8
  
  return (r, g, b, a)

def get_interpolated_cmpr_colors(color_0_rgb565, color_1_rgb565):
  color_0 = convert_rgb565_to_color(color_0_rgb565)
  color_1 = convert_rgb565_to_color(color_1_rgb565)
  r0, g0, b0, _ = color_0
  r1, g1, b1, _ = color_1
  if color_0_rgb565 > color_1_rgb565:
    color_2 = (
      (2*r0 + 1*r1)//3,
      (2*g0 + 1*g1)//3,
      (2*b0 + 1*b1)//3,
      255
    )
    color_3 = (
      (1*r0 + 2*r1)//3,
      (1*g0 + 2*g1)//3,
      (1*b0 + 2*b1)//3,
      255
    )
  else:
    color_2 = (r0//2+r1//2, g0//2+g1//2, b0//2+b1//2, 255)
    color_3 = (0, 0, 0, 0)
  colors = [color_0, color_1, color_2, color_3]
  return colors

def average_colors_together(colors):
  transparent_color = next(((r,g,b,a) for r,g,b,a in colors if a == 0), None)
  if transparent_color:
    # Need to ensure a fully transparent color exists in the final palette if one existed originally.
    return transparent_color
  
  r_sum = sum(r for r,g,b,a in colors)
  g_sum = sum(g for r,g,b,a in colors)
  b_sum = sum(b for r,g,b,a in colors)
  a_sum = sum(a for r,g,b,a in colors)
  
  average_color = (
    r_sum//len(colors),
    g_sum//len(colors),
    b_sum//len(colors),
    a_sum//len(colors),
  )
  
  return average_color


def decode_palettes(palette_data, palette_format, num_colors, image_format):
  if not isinstance(image_format, ImageFormat):
    raise Exception("Invalid image format: %s" % image_format)
  if image_format not in IMAGE_FORMATS_THAT_USE_PALETTES:
    return []
  
  colors = []
  offset = 0
  for i in range(num_colors):
    raw_color = read_u16(palette_data, offset)
    color = decode_color(raw_color, palette_format)
    colors.append(color)
    offset += 2
  
  return colors

def decode_color(raw_color, palette_format):
  if palette_format == PaletteFormat.IA8:
    color = convert_ia8_to_color(raw_color)
  elif palette_format == PaletteFormat.RGB565:
    color = convert_rgb565_to_color(raw_color)
  elif palette_format == PaletteFormat.RGB5A3:
    color = convert_rgb5a3_to_color(raw_color)
  
  return color

def decode_image(image_data, palette_data, image_format, palette_format, num_colors, image_width, image_height):
  colors = decode_palettes(palette_data, palette_format, num_colors, image_format)
  
  block_width = BLOCK_WIDTHS[image_format]
  block_height = BLOCK_HEIGHTS[image_format]
  block_data_size = BLOCK_DATA_SIZES[image_format]
  
  image = Image.new("RGBA", (image_width, image_height), (0, 0, 0, 0))
  pixels = image.load()
  offset = 0
  block_x = 0
  block_y = 0
  while block_y < image_height:
    pixel_color_data = decode_block(image_format, image_data, offset, block_data_size, colors)
    
    for i, color in enumerate(pixel_color_data):
      x_in_block = i % block_width
      y_in_block = i // block_width
      x = block_x+x_in_block
      y = block_y+y_in_block
      if x >= image_width or y >= image_height:
        continue
      
      pixels[x,y] = color
    
    offset += block_data_size
    block_x += block_width
    if block_x >= image_width:
      block_x = 0
      block_y += block_height
  
  return image

def decode_block(image_format, image_data, offset, block_data_size, colors):
  if image_format == ImageFormat.I4:
    return decode_i4_block(image_format, image_data, offset, block_data_size, colors)
  elif image_format == ImageFormat.I8:
    return decode_i8_block(image_format, image_data, offset, block_data_size, colors)
  elif image_format == ImageFormat.IA4:
    return decode_ia4_block(image_format, image_data, offset, block_data_size, colors)
  elif image_format == ImageFormat.IA8:
    return decode_ia8_block(image_format, image_data, offset, block_data_size, colors)
  elif image_format == ImageFormat.RGB565:
    return decode_rgb565_block(image_format, image_data, offset, block_data_size, colors)
  elif image_format == ImageFormat.RGB5A3:
    return decode_rgb5a3_block(image_format, image_data, offset, block_data_size, colors)
  elif image_format == ImageFormat.RGBA32:
    return decode_rgba32_block(image_format, image_data, offset, block_data_size, colors)
  elif image_format == ImageFormat.C4:
    return decode_c4_block(image_format, image_data, offset, block_data_size, colors)
  elif image_format == ImageFormat.C8:
    return decode_c8_block(image_format, image_data, offset, block_data_size, colors)
  elif image_format == ImageFormat.C14X2:
    return decode_c14x2_block(image_format, image_data, offset, block_data_size, colors)
  elif image_format == ImageFormat.CMPR:
    return decode_cmpr_block(image_format, image_data, offset, block_data_size, colors)
  else:
    raise Exception("Unknown image format: %s" % image_format.name)

def decode_i4_block(image_format, image_data, offset, block_data_size, colors):
  pixel_color_data = []
  
  for byte_index in range(block_data_size):
    byte = read_u8(image_data, offset+byte_index)
    for nibble_index in range(2):
      i4 = (byte >> (1-nibble_index)*4) & 0xF
      color = convert_i4_to_color(i4)
      
      pixel_color_data.append(color)
  
  return pixel_color_data

def decode_i8_block(image_format, image_data, offset, block_data_size, colors):
  pixel_color_data = []
  
  for i in range(block_data_size):
    i8 = read_u8(image_data, offset+i)
    color = convert_i8_to_color(i8)
    
    pixel_color_data.append(color)
  
  return pixel_color_data

def decode_ia4_block(image_format, image_data, offset, block_data_size, colors):
  pixel_color_data = []
  
  for i in range(block_data_size):
    ia4 = read_u8(image_data, offset+i)
    color = convert_ia4_to_color(ia4)
    
    pixel_color_data.append(color)
  
  return pixel_color_data

def decode_ia8_block(image_format, image_data, offset, block_data_size, colors):
  pixel_color_data = []
  
  for i in range(block_data_size//2):
    ia8 = read_u16(image_data, offset+i*2)
    color = convert_ia8_to_color(ia8)
    
    pixel_color_data.append(color)
  
  return pixel_color_data

def decode_rgb565_block(image_format, image_data, offset, block_data_size, colors):
  pixel_color_data = []
  
  for i in range(block_data_size//2):
    rgb565 = read_u16(image_data, offset+i*2)
    color = convert_rgb565_to_color(rgb565)
    
    pixel_color_data.append(color)
  
  return pixel_color_data

def decode_rgb5a3_block(image_format, image_data, offset, block_data_size, colors):
  pixel_color_data = []
  
  for i in range(block_data_size//2):
    rgb5a3 = read_u16(image_data, offset+i*2)
    color = convert_rgb5a3_to_color(rgb5a3)
    
    pixel_color_data.append(color)
  
  return pixel_color_data

def decode_rgba32_block(image_format, image_data, offset, block_data_size, colors):
  pixel_color_data = []
  
  for i in range(16):
    a = read_u8(image_data, offset+(i*2))
    r = read_u8(image_data, offset+(i*2)+1)
    g = read_u8(image_data, offset+(i*2)+32)
    b = read_u8(image_data, offset+(i*2)+33)
    color = (r, g, b, a)
    
    pixel_color_data.append(color)
  
  return pixel_color_data

def decode_c4_block(image_format, image_data, offset, block_data_size, colors):
  pixel_color_data = []
  
  for byte_index in range(block_data_size):
    byte = read_u8(image_data, offset+byte_index)
    for nibble_index in range(2):
      color_index = (byte >> (1-nibble_index)*4) & 0xF
      if color_index >= len(colors):
        # This block bleeds past the edge of the image
        color = None
      else:
        color = colors[color_index]
      
      pixel_color_data.append(color)
  
  return pixel_color_data

def decode_c8_block(image_format, image_data, offset, block_data_size, colors):
  pixel_color_data = []
  
  for i in range(block_data_size):
    color_index = read_u8(image_data, offset+i)
    if color_index >= len(colors):
      # This block bleeds past the edge of the image
      color = None
    else:
      color = colors[color_index]
    
    pixel_color_data.append(color)
  
  return pixel_color_data

def decode_c14x2_block(image_format, image_data, offset, block_data_size, colors):
  pixel_color_data = []
  
  for i in range(block_data_size//2):
    color_index = read_u16(image_data, offset+i*2) & 0x3FFF
    if color_index >= len(colors):
      # This block bleeds past the edge of the image
      color = None
    else:
      color = colors[color_index]
    
    pixel_color_data.append(color)
  
  return pixel_color_data

def decode_cmpr_block(image_format, image_data, offset, block_data_size, colors):
  pixel_color_data = [None]*64
  
  subblock_offset = offset
  for subblock_index in range(4):
    subblock_x = (subblock_index%2)*4
    subblock_y = (subblock_index//2)*4
    
    color_0_rgb565 = read_u16(image_data, subblock_offset)
    color_1_rgb565 = read_u16(image_data, subblock_offset+2)
    colors = get_interpolated_cmpr_colors(color_0_rgb565, color_1_rgb565)
    
    color_indexes = read_u32(image_data, subblock_offset+4)
    for i in range(16):
      color_index = ((color_indexes >> ((15-i)*2)) & 3)
      color = colors[color_index]
      
      x_in_subblock = i % 4
      y_in_subblock = i // 4
      pixel_index_in_block = subblock_x + subblock_y*8 + y_in_subblock*8 + x_in_subblock
      
      pixel_color_data[pixel_index_in_block] = color
    
    subblock_offset += 8
  
  return pixel_color_data

class InvalidOffsetError(Exception):
  pass

def read_all_bytes(data):
  data.seek(0)
  return data.read()

def read_bytes(data, offset, length):
  data.seek(offset)
  return data.read(length)

def read_str(data, offset, length):
  data_length = data.seek(0, 2)
  if offset+length > data_length:
    raise InvalidOffsetError("Offset 0x%X, length 0x%X is past the end of the data (length 0x%X)." % (offset, length, data_length))
  data.seek(offset)
  string = data.read(length).decode("shift_jis")
  string = string.rstrip("\0") # Remove trailing null bytes
  return string

def try_read_str(data, offset, length):
  try:
    return read_str(data, offset, length)
  except UnicodeDecodeError:
    return None
  except InvalidOffsetError:
    return None

def read_u8(data, offset):
  data.seek(offset)
  return struct.unpack(">B", data.read(1))[0]

def read_u16(data, offset):
  data.seek(offset)
  return struct.unpack(">H", data.read(2))[0]

def read_u32(data, offset):
  data.seek(offset)
  return struct.unpack(">I", data.read(4))[0]

def write_u16(data, offset, new_value):
  new_value = struct.pack(">H", new_value)
  data.seek(offset)
  data.write(new_value)

PY_FAST_YAZ0_INSTALLED = False

class Yaz0:
  @staticmethod
  def check_is_compressed(data):
    if try_read_str(data, 0, 4) != "Yaz0":
      return False
    
    return True
  
  @staticmethod
  def decompress(comp_data):
    if not Yaz0.check_is_compressed(comp_data):
      print("File is not compressed.")
      return comp_data
    
    uncomp_size = read_u32(comp_data, 4)
    comp_size = comp_data.seek(0, 2)
    
    comp = read_all_bytes(comp_data)
    
    output = []
    output_len = 0
    src_offset = 0x10
    valid_bit_count = 0
    curr_code_byte = 0
    while output_len < uncomp_size:
      if valid_bit_count == 0:
        curr_code_byte = comp[src_offset]
        src_offset += 1
        valid_bit_count = 8
      
      if curr_code_byte & 0x80 != 0:
        output.append(comp[src_offset])
        src_offset += 1
        output_len += 1
      else:
        byte1 = comp[src_offset]
        byte2 = comp[src_offset+1]
        src_offset += 2
        
        dist = ((byte1&0xF) << 8) | byte2
        copy_src_offset = output_len - (dist + 1)
        num_bytes = (byte1 >> 4)
        if num_bytes == 0:
          num_bytes = comp[src_offset] + 0x12
          src_offset += 1
        else:
          num_bytes += 2
        
        for i in range(0, num_bytes):
          output.append(output[copy_src_offset])
          output_len += 1
          copy_src_offset += 1
      
      curr_code_byte = (curr_code_byte << 1)
      valid_bit_count -= 1
    
    uncomp_data = struct.pack("B"*output_len, *output)
    
    return BytesIO(uncomp_data)

# Simple script wrapper to decode a BTI image
if __name__ == '__main__':
    print("DECODE BTI " + sys.argv[1])
    file_data = open(sys.argv[1], "rb")
    bti = BTIFile(file_data)
    bti.render().save(sys.argv[1] + ".png", "PNG")
