use rusty_ffmpeg::ffi::{
  AVCodec,
  AVCodecContext,
  AVCodecParserContext,
  AVFrame,
  AVPacket,
  avcodec_alloc_context3,
  avcodec_find_decoder,
  avcodec_open2,
  avcodec_receive_frame,
  avcodec_send_packet,
  av_frame_alloc,
  av_frame_free,
  av_packet_alloc,
  av_parser_init,
  av_parser_parse2,
  av_packet_free,
  avcodec_free_context,
  AV_INPUT_BUFFER_PADDING_SIZE,
  AVCodecID_AV_CODEC_ID_H264 as AV_CODEC_ID_H264,
};

use rusty_ffmpeg::avutil::error::{
  AVERROR,
  AVERROR_EOF,
};

use libc::EAGAIN;

use std::env;
use std::fs::File;
use std::io::{
  Read,
  BufReader,
  Write,
};
use std::path::Path;
use std::slice;

// Somehow doesn't exist in the binding...
const AV_NOPTS_VALUE: i64 = 9223372036854775808u64 as i64;

fn pgm_save(buffer: &[u8], wrap: usize, xsize: usize, ysize: usize,
            filename: &String) -> Result<(), std::io::Error> {
    let mut file = File::create(filename)?;
    let data = format!("P5\n{} {}\n{}\n", xsize, ysize, 255);
    file.write_all(data.as_bytes())?;

    for i in 0..ysize {
        file.write_all(&buffer[i * wrap..(i * wrap + xsize)])?;
    }

    Ok(())
}

fn decode(context: *mut AVCodecContext, frame: &mut AVFrame,
          packet: *const AVPacket,
          filename: &String) -> Result<(), String> {
    let mut ret = unsafe { avcodec_send_packet(context, packet) } ;
    if ret < 0 {
        return Err(String::from("Unable send packet."));
    }
    while ret >= 0 {
        ret = unsafe { avcodec_receive_frame(context, frame) };
        // Following error should ignored.
        if ret == AVERROR(EAGAIN) || ret == AVERROR_EOF {
            break;
        } else if ret < 0 {
            return Err(String::from("Unable to get frame."));
        }
        let width = (*frame).width as usize;
        let height = (*frame).height as usize;
        let wrap = (*frame).linesize[0] as usize;
        let data = unsafe { slice::from_raw_parts((*frame).data[0], wrap * height) };
        
        let filename = format!("{}{}.pgm", (*filename), unsafe{ (*context).frame_number });

        pgm_save(data, wrap, width, height, &filename).unwrap();
    }
 
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    const INPUT_BUFFER_SIZE:usize = 4096;

    let args: Vec<String> = env::args().collect();
    let path = Path::new(&args[1]);

    let file = File::open(&path).expect("Unable to open input");
  
    let codec: *mut AVCodec = unsafe { avcodec_find_decoder(AV_CODEC_ID_H264) };
    if codec.is_null() {
        panic!("Unable to create codec.");
    }
    let context: *mut AVCodecContext = unsafe { avcodec_alloc_context3(codec) };
    if context.is_null() {
        panic!("Unable to create for codec.");
    }
    if {
        unsafe { avcodec_open2(context, codec, std::ptr::null_mut()) }
    } < 0 {
        panic!("Unable to open codec.");
    }

    let packet: *mut AVPacket = unsafe { av_packet_alloc() };
    if packet.is_null() {
        panic!("Unable to create packet.");
    }

    let parser: *mut AVCodecParserContext = unsafe {
        av_parser_init((*codec).id as i32)
    };
    if parser.is_null() {
        panic!("Unable to initialize the parser");
    }

    let frame = unsafe { av_frame_alloc().as_mut() }.expect("Unable to create frame");

    let mut reader = BufReader::new(file);
    let mut buffer = [0; INPUT_BUFFER_SIZE + AV_INPUT_BUFFER_PADDING_SIZE as usize];
    loop {
        match reader.read(&mut buffer)? {
            0 => {
               unsafe { 
                        let ret = av_parser_parse2(parser, context,
                                                   &mut (*packet).data,
                                                   &mut (*packet).size,
                                                   &buffer[0],
                                                   0,
                                                   AV_NOPTS_VALUE,
                                                   AV_NOPTS_VALUE, 0);
                    if (*packet).size > 0 {
                        decode(context, frame, packet, &args[2]).unwrap();
                    }
                }
                break;
                },
            n => {
                let buffer = &buffer[..n];
                let mut size = n as i32;
                let mut index = 0;
                while size > 0 {
                    unsafe {
                        let ret = av_parser_parse2(parser, context,
                                                   &mut (*packet).data,
                                                   &mut (*packet).size,
                                                   &buffer[index as usize],
                                                   size,
                                                   AV_NOPTS_VALUE,
                                                   AV_NOPTS_VALUE, 0);
                        if ret < 0 { break; } 
                        size -= ret as i32;
                        index += ret;
                        if (*packet).size > 0 {
                          decode(context, frame, packet, &args[2]).unwrap();
                        }
                    }
                }
            }
        };
    }

    unsafe {
        av_packet_free(&mut (packet as *mut _));
        av_frame_free(&mut (frame as *mut _));
        avcodec_free_context(&mut (context as *mut _));
    }

    Ok(())
}
