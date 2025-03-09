```bash
#!/bin/bash
input="result.png"

width=122
height=250

bytes_per_row=$(( (width + 7) / 8 )) # rounds up
total_bytes=$(( bytes_per_row * height ))

red="$(convert "$input" -fill black -opaque "#FFFFFF" -negate -monochrome -depth 1 gray:- | xxd -i)"
black="$(convert "$input" -monochrome -depth 1 gray:- | xxd -i)"
printf "#include \"imagedata.h\" \n" > tempimage
printf "#include <avr/pgmspace.h> \n" >> tempimage
printf "const unsigned char gImage_2in13b_V4b[$total_bytes] PROGMEM = { \n" >> tempimage
printf "%s\n" "$black" >> tempimage
printf "};\n" >> tempimage
printf "const unsigned char gImage_2in13b_V4r[$total_bytes] PROGMEM = { \n" >> tempimage
printf "%s\n" "$red" >> tempimage
printf "};\n" >> tempimage
```

