use image::codecs::bmp::{BmpDecoder, BmpEncoder};
use image::{
    DynamicImage, EncodableLayout, GenericImageView, GrayImage, ImageBuffer, ImageDecoder,
    ImageReader, Pixel, RgbImage, Rgba, RgbaImage,
};
use quick_js::{Context, JsValue};
use std::fs::File;
use std::io::BufReader;

// Helper function to read an image and convert it to RGBA8
fn read_image_rgb8(filename: &str) -> Result<RgbaImage, image::ImageError> {
    let image_file = File::open(filename).unwrap();
    let image_buffer = BufReader::new(image_file);
    let image = ImageReader::new(image_buffer)
        .with_guessed_format()?
        .decode()?;
    Ok(image.to_rgba8())
}

// Helper function to convert RGBA8 image to a Vec<u8> for Javascript
fn image_to_vec(image: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> Vec<u8> {
    image.to_vec()
}

// Helper function to create an RGBA8 image from a Vec<u8>
fn vec_to_image(data: Vec<u8>, width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_raw(width, height, data).unwrap()
}

struct Output {
    combined: RgbImage,
    red_only: RgbImage,
    black_only: RgbImage,
}

#[allow(clippy::too_many_lines)]
fn dither_image(input: &RgbaImage) -> Result<Output, Box<dyn std::error::Error>> {
    let width = input.width();
    let height = input.height();
    let image_data = image_to_vec(input);

    let context = Context::new()?;

    let width_js = JsValue::Int(width as i32);
    let height_js = JsValue::Int(height as i32);

    let image_data = JsValue::Array(
        image_data
            .into_iter()
            .map(|x| JsValue::Int(x.into()))
            .collect::<Vec<_>>(),
    );

    context.set_global("sourceImageData", image_data)?;
    context.set_global("sW", width_js)?;
    context.set_global("sH", height_js)?;

    let script = r"
        var curPal = [[0,0,0],[255,255,255],[127,0,0]];
    
        function addVal(c,r,g,b,k){
            return[c[0]+(r*k)/32,c[1]+(g*k)/32,c[2]+(b*k)/32];
        }

        function getErr(r,g,b,stdCol){
            r-=stdCol[0];
            g-=stdCol[1];
            b-=stdCol[2];
            return r*r + g*g + b*b;
        }

        function setVal(p,i,c){
            p.data[i]=curPal[c][0];
            p.data[i+1]=curPal[c][1];
            p.data[i+2]=curPal[c][2];
            p.data[i+3]=255;
        }


        function getNear(r,g,b){
            var ind=0;
            var err=getErr(r,g,b,curPal[0]);
            for (var i=1;i<curPal.length;i++)
            {
                var cur=getErr(r,g,b,curPal[i]);
                if (cur<err){err=cur;ind=i;}
            }
            return ind;
        }            

        function procImg() {
            var dX = 0;
            var dY = 0;
            var dW = sW;
            var dH = sH;


            var pSrc = { data: sourceImageData };
            var pDst = { data: new Array(sW * sH * 4) };

            var index = 0;
            var aInd=0;
            var bInd=1;
            var errArr=new Array(2);
            errArr[0]=new Array(dW);
            errArr[1]=new Array(dW);
            for (var i=0;i<dW;i++)
                errArr[bInd][i]=[0,0,0];

            for (var j=0;j<dH;j++){
                var y=dY+j;
                if ((y<0)||(y>=sH)){
                    for (var i=0;i<dW;i++,index+=4)setVal(pDst,index,(i+j)%2==0?1:0);  
                    continue;
                }
                aInd=((bInd=aInd)+1)&1;
                for (var i=0;i<dW;i++)errArr[bInd][i]=[0,0,0];
                for (var i=0;i<dW;i++){
                    var x=dX+i;
                    if ((x<0)||(x>=sW)){
                        setVal(pDst,index,(i+j)%2==0?1:0);
                        index+=4;
                        continue;
                    }
                    var pos=(y*sW+x)*4;
                    var old=errArr[aInd][i];
                    var r=pSrc.data[pos  ]+old[0];
                    var g=pSrc.data[pos+1]+old[1];
                    var b=pSrc.data[pos+2]+old[2];
                    var colVal = curPal[getNear(r,g,b)];
                    pDst.data[index++]=colVal[0];
                    pDst.data[index++]=colVal[1];
                    pDst.data[index++]=colVal[2];
                    pDst.data[index++]=255;
                    r=(r-colVal[0]);
                    g=(g-colVal[1]);
                    b=(b-colVal[2]);
                    if (i==0){
                        errArr[bInd][i  ]=addVal(errArr[bInd][i  ],r,g,b,7.0);
                        errArr[bInd][i+1]=addVal(errArr[bInd][i+1],r,g,b,2.0);
                        errArr[aInd][i+1]=addVal(errArr[aInd][i+1],r,g,b,7.0);
                    } else if (i==dW-1){
                        errArr[bInd][i-1]=addVal(errArr[bInd][i-1],r,g,b,7.0);
                        errArr[bInd][i  ]=addVal(errArr[bInd][i  ],r,g,b,9.0);
                    } else{
                        errArr[bInd][i-1]=addVal(errArr[bInd][i-1],r,g,b,3.0);
                        errArr[bInd][i  ]=addVal(errArr[bInd][i  ],r,g,b,5.0);
                        errArr[bInd][i+1]=addVal(errArr[bInd][i+1],r,g,b,1.0);
                        errArr[aInd][i+1]=addVal(errArr[aInd][i+1],r,g,b,7.0);
                    }
                }
            }
            return pDst.data;
        }
        ";

    let full_script = format!("{script}; procImg()");

    let JsValue::Array(data) = context.eval(&full_script)? else {
        panic!("must be array");
    };

    let data = data
        .into_iter()
        .map(|x| {
            let JsValue::Int(x) = x else {
                panic!("nah");
            };
            x as u8
        })
        .collect::<Vec<_>>();

    let processed_image = vec_to_image(data, width, height);

    let mut image = vec![];

    processed_image.write_to(
        &mut std::io::Cursor::new(&mut image),
        image::ImageFormat::Png,
    )?;

    let image =
        ImageReader::with_format(std::io::Cursor::new(image), image::ImageFormat::Png).decode()?;

    let mut red_image = image.clone();

    if let Some(pixels) = red_image.as_mut_rgba8() {
        for pixel in pixels.pixels_mut() {
            let [r, g, b, _] = &mut pixel.0;
            if *r == 127 {
                *r = 0;
                *g = 0;
                *b = 0;
                continue;
            }
            if *r == 0 && *g == 0 && *b == 0 {
                *r = 255;
                *g = 255;
                *b = 255;
            }
        }
    }

    let mut black_image = image.clone();

    if let Some(pixels) = black_image.as_mut_rgba8() {
        for pixel in pixels.pixels_mut() {
            let [r, g, b, _] = &mut pixel.0;
            if *r == 127 {
                *r = 0;
                *g = 0;
                *b = 0;
                continue;
            }
        }
    }
    Ok(Output {
        combined: image.to_rgb8(),
        red_only: red_image.to_rgb8(),
        black_only: black_image.to_rgb8(),
    })
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = read_image_rgb8("input.png")?;

    let Output {
        combined,
        red_only,
        black_only,
    } = dither_image(&input)?;

    red_only.save("red_image.png")?;
    black_only.save("black_image.png")?;
    combined.save("result.png")?;

    Ok(())
}
