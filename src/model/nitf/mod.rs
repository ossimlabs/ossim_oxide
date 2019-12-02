//! NITF related module

use std::io::prelude::*;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::mpsc::channel;
use std::fs::File;

use rayon::prelude::*;

use crate::base::Model;

/// NITF (National Imagery Transmission Format) model
pub struct NITF {
    metadata: NITFmetadata
}


struct NITFmetadata {
    file_header: BTreeMap<String,String>,
    image_subheaders: Vec<BTreeMap<String, String>>,
    graphic_subheaders: Vec<BTreeMap<String, String>>,
    text_subheaders: Vec<BTreeMap<String, String>>,
    data_ext_subheaders: Vec<BTreeMap<String, String>>
}


impl Model for NITF {

    type MyType = NITF;

    /// Returns a Model for the given NITF file
    ///
    /// # Arguments
    ///
    /// * `filename` - A string of the path to the nitf file.
    ///
    /// # Examples
    /// ```
    /// use ossim_oxide::base::Model;
    /// use ossim_oxide::model::nitf;
    /// let myNitf = NITF::new("/path/to/nitf/file.NTF");
    /// ```
    fn new(filename: String) -> std::io::Result<NITF> {

        let mut file = File::open(filename)?;
        let nitf = &mut Vec::new();
        file.read_to_end(nitf).unwrap();
        drop(file);

        let file_header = NITF::parse_header(&nitf).unwrap();

        let mut offset = file_header.get("HL").unwrap().parse::<usize>().unwrap();

        // Calculate the offset to each image header
        let num_of_image_seg = file_header.get("NUMI").unwrap().parse::<usize>().unwrap();
        let mut image_offsets = Vec::new();
        for i in 1..=num_of_image_seg {
            image_offsets.push(offset);
            offset += file_header.get(&format!("LISH{:03}",i)).unwrap().parse::<usize>().unwrap() +
                    file_header.get(&format!("LI{:03}",i)).unwrap().parse::<usize>().unwrap();
        }

        // Sync up return values of parallel parsing of image headers
        let (img_sender, img_receiver) = channel();
        image_offsets.into_par_iter().for_each_with(img_sender, |s, offset| s.send(NITF::parse_image_subheader(&nitf, offset).unwrap()).unwrap());
        let image_subheaders: Vec<_> = img_receiver.iter().collect();

        let num_of_graphic_seg = file_header.get("NUMS").unwrap().parse::<usize>().unwrap();
        let mut graphic_offsets = Vec::new();
        for i in 1..=num_of_graphic_seg {
            graphic_offsets.push(offset);
            offset += file_header.get(&format!("LSSH{:03}",i)).unwrap().parse::<usize>().unwrap() +
                    file_header.get(&format!("LS{:03}",i)).unwrap().parse::<usize>().unwrap();
        }

        let (graphic_sender, graphic_receiver) = channel();
        graphic_offsets.into_par_iter().for_each_with(graphic_sender, |s, offset| s.send(NITF::parse_graphic_subheader(&nitf, offset).unwrap()).unwrap());
        let graphic_subheaders: Vec<_> = graphic_receiver.iter().collect();

        let num_of_text_seg = file_header.get("NUMT").unwrap().parse::<usize>().unwrap();
        let mut text_offsets = Vec::new();
        for i in 1..=num_of_text_seg {
            text_offsets.push(offset);
            offset += file_header.get(&format!("LTSH{:03}",i)).unwrap().parse::<usize>().unwrap() +
                    file_header.get(&format!("LT{:03}",i)).unwrap().parse::<usize>().unwrap();
        }

        let (text_sender, text_receiver) = channel();
        text_offsets.into_par_iter().for_each_with(text_sender, |s, offset| s.send(NITF::parse_text_subheader(&nitf, offset).unwrap()).unwrap());
        let text_subheaders: Vec<_> = text_receiver.iter().collect();

        let num_of_data_ext_seg = file_header.get("NUMDES").unwrap().parse::<usize>().unwrap();
        let mut data_ext_offsets = Vec::new();
        for i in 1..=num_of_data_ext_seg {
            data_ext_offsets.push(offset);
            offset += file_header.get(&format!("LDSH{:03}",i)).unwrap().parse::<usize>().unwrap() +
                    file_header.get(&format!("LD{:03}",i)).unwrap().parse::<usize>().unwrap();
        }

        let (data_sender, data_receiver) = channel();
        data_ext_offsets.into_par_iter().for_each_with(data_sender, |s, offset| s.send(NITF::parse_data_ext_seg_subheader(&nitf, offset).unwrap()).unwrap());
        let data_ext_subheaders: Vec<_> = data_receiver.iter().collect();


        let metadata = NITFmetadata {
            file_header: file_header,
            image_subheaders: image_subheaders,
            graphic_subheaders: graphic_subheaders,
            text_subheaders: text_subheaders,
            data_ext_subheaders: data_ext_subheaders
        };

        Ok(NITF {
            metadata: metadata
        })

    }
}


impl fmt::Display for NITF {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut retval = "".to_string();
        for (field, value) in &self.metadata.file_header {
            retval = format!("{}\nNITF::{}: {}", retval, field, value);
        }
        for (index, image_subheader) in (&self.metadata.image_subheaders).iter().enumerate() {
            for (field, value) in image_subheader {
                retval = format!("{}\nNITF::IMAGE{:03}::{}: {}", retval, index, field, value);
            }
        }
        for (index, graphic_subheader) in (&self.metadata.graphic_subheaders).iter().enumerate() {
            for (field, value) in graphic_subheader {
                retval = format!("{}\nNITF::GRAPHIC{:03}::{}: {}", retval, index, field, value);
            }
        }
        for (index, text_subheader) in (&self.metadata.text_subheaders).iter().enumerate() {
            for (field, value) in text_subheader {
                retval = format!("{}\nNITF::TEXT{:03}::{}: {}", retval, index, field, value);
            }
        }
        for (index, data_ext_subheader) in (&self.metadata.data_ext_subheaders).iter().enumerate() {
            for (field, value) in data_ext_subheader {
                retval = format!("{}\nNITF::DES{:03}::{}: {}", retval, index, field, value);
            }
        }
        write!(f, "{}", retval)
    }
}


impl NITF {



    fn parse_header(nitf: &Vec<u8>) -> std::io::Result<BTreeMap<String,String>> {

        let mut cursor = 0;

        let mut file_header = BTreeMap::new();

        // File Profile Name
        file_header.insert("FHDR".to_string(),String::from_utf8(nitf[cursor..cursor+4].to_vec()).unwrap());
        cursor = cursor + 4;

        // File Version
        file_header.insert("FVER".to_string(),String::from_utf8(nitf[cursor..cursor+5].to_vec()).unwrap());
        cursor = cursor + 5;

        // Complexity Level
        file_header.insert("CLEVEL".to_string(),String::from_utf8(nitf[cursor..cursor+2].to_vec()).unwrap());
        cursor = cursor + 2;

        // Standard Type
        file_header.insert("STYPE".to_string(),String::from_utf8(nitf[cursor..cursor+4].to_vec()).unwrap());
        cursor = cursor + 4;

        // Originating Station ID
        file_header.insert("OSTAID".to_string(),String::from_utf8(nitf[cursor..cursor+10].to_vec()).unwrap().trim().to_string());
        cursor = cursor + 10;

        // File Data and Time
        file_header.insert("FDT".to_string(),
            // Year
            String::from_utf8(nitf[cursor..cursor+4].to_vec()).unwrap() + "/" +
            // Month
            &String::from_utf8(nitf[cursor+4..cursor+6].to_vec()).unwrap() + "/" +
            // Day
            &String::from_utf8(nitf[cursor+6..cursor+8].to_vec()).unwrap() + " " +
            // Hour
            &String::from_utf8(nitf[cursor+8..cursor+10].to_vec()).unwrap() + ":" +
            // Minute
            &String::from_utf8(nitf[cursor+10..cursor+12].to_vec()).unwrap() + ":" +
            // Second
            &String::from_utf8(nitf[cursor+12..cursor+14].to_vec()).unwrap()
        );
        cursor = cursor + 14;

        // File Title
        if !String::from_utf8(nitf[cursor..cursor+80].to_vec()).unwrap().trim().to_string().is_empty() {
            file_header.insert("FTITLE".to_string(),String::from_utf8(nitf[cursor..cursor+80].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 80;

        // File Security Classification
        file_header.insert("FSCLAS".to_string(),String::from_utf8(nitf[cursor..cursor+1].to_vec()).unwrap());
        cursor = cursor + 1;

        // File Secruity Classification System
        if !String::from_utf8(nitf[cursor..cursor+2].to_vec()).unwrap().trim().to_string().is_empty() {
            file_header.insert("FSCLSY".to_string(),String::from_utf8(nitf[cursor..cursor+2].to_vec()).unwrap());
        }
        cursor = cursor + 2;

        // File Codewords
        if !String::from_utf8(nitf[cursor..cursor+11].to_vec()).unwrap().trim().to_string().is_empty() {
            file_header.insert("FSCODE".to_string(),String::from_utf8(nitf[cursor..cursor+11].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 11;

        // File Control and Handling
        if !String::from_utf8(nitf[cursor..cursor+2].to_vec()).unwrap().trim().to_string().is_empty() {
            file_header.insert("FSCTLH".to_string(),String::from_utf8(nitf[cursor..cursor+2].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 2;

        // File Releasing Instructions
        if !String::from_utf8(nitf[cursor..cursor+20].to_vec()).unwrap().trim().to_string().is_empty() {
            file_header.insert("FSREL".to_string(),String::from_utf8(nitf[cursor..cursor+20].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 20;

        // File Declassification type
        if !String::from_utf8(nitf[cursor..cursor+2].to_vec()).unwrap().trim().to_string().is_empty() {
            file_header.insert("FSDCTP".to_string(),String::from_utf8(nitf[cursor..cursor+2].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 2;

        // File Declassification Date
        if !String::from_utf8(nitf[cursor..cursor+8].to_vec()).unwrap().trim().to_string().is_empty() {
            file_header.insert("FSDCDT".to_string(),
                // Year
                (String::from_utf8(nitf[cursor..cursor+4].to_vec()).unwrap() + "/" +
                // Month
                &String::from_utf8(nitf[cursor+4..cursor+6].to_vec()).unwrap() + "/" +
                // Day
                &String::from_utf8(nitf[cursor+6..cursor+8].to_vec()).unwrap()).trim().to_string()

            );
        }
        cursor = cursor + 8;

        // File Declassification Exemption
        if !String::from_utf8(nitf[cursor..cursor+4].to_vec()).unwrap().trim().to_string().is_empty() {
            file_header.insert("FSDCXM".to_string(),String::from_utf8(nitf[cursor..cursor+4].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 4;

        // File Downgrade
        if !String::from_utf8(nitf[cursor..cursor+1].to_vec()).unwrap().trim().to_string().is_empty() {
            file_header.insert("FSDG".to_string(),String::from_utf8(nitf[cursor..cursor+1].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 1;

        // File Downgrade Date
        if !String::from_utf8(nitf[cursor..cursor+8].to_vec()).unwrap().trim().to_string().is_empty() {
            file_header.insert("FSDGDT".to_string(),
                // Year
                String::from_utf8(nitf[cursor..cursor+4].to_vec()).unwrap() + "/" +
                // Month
                &String::from_utf8(nitf[cursor+4..cursor+6].to_vec()).unwrap() + "/" +
                // Day
                &String::from_utf8(nitf[cursor+6..cursor+8].to_vec()).unwrap()

            );
        }
        cursor = cursor + 8;

        // File Classification Text
        if !String::from_utf8(nitf[cursor..cursor+43].to_vec()).unwrap().trim().to_string().is_empty() {
            file_header.insert("FSCLTX".to_string(),String::from_utf8(nitf[cursor..cursor+43].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 43;

        // File Classification Authority Type
        if !String::from_utf8(nitf[cursor..cursor+1].to_vec()).unwrap().trim().to_string().is_empty() {
            file_header.insert("FSCATP".to_string(),String::from_utf8(nitf[cursor..cursor+1].to_vec()).unwrap());
        }
        cursor = cursor + 1;

        // File Classification Authority
        if !String::from_utf8(nitf[cursor..cursor+40].to_vec()).unwrap().trim().to_string().is_empty() {
            file_header.insert("FSCAUT".to_string(),String::from_utf8(nitf[cursor..cursor+40].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 40;

        // File Classification Reason
        if !String::from_utf8(nitf[cursor..cursor+1].to_vec()).unwrap().trim().to_string().is_empty() {
            file_header.insert("FSCRSN".to_string(),String::from_utf8(nitf[cursor..cursor+1].to_vec()).unwrap());
        }
        cursor = cursor + 1;

        // File Security Source Date
        if !String::from_utf8(nitf[cursor..cursor+8].to_vec()).unwrap().trim().to_string().is_empty() {
            file_header.insert("FSSRDT".to_string(),
                // Year
                (String::from_utf8(nitf[cursor..cursor+4].to_vec()).unwrap() + "/" +
                // Month
                &String::from_utf8(nitf[cursor+4..cursor+6].to_vec()).unwrap() + "/" +
                // Day
                &String::from_utf8(nitf[cursor+6..cursor+8].to_vec()).unwrap()).trim().to_string()

            );
        }
        cursor = cursor + 8;

        // File Security Control Number
        if !String::from_utf8(nitf[cursor..cursor+15].to_vec()).unwrap().trim().to_string().is_empty() {
            file_header.insert("FSCLTN".to_string(),String::from_utf8(nitf[cursor..cursor+15].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 15;

        // File Copy Number
        file_header.insert("FSCOP".to_string(),String::from_utf8(nitf[cursor..cursor+5].to_vec()).unwrap().trim().to_string());
        cursor = cursor + 5;

        // File Number of Copies
        file_header.insert("FSCPYS".to_string(),String::from_utf8(nitf[cursor..cursor+5].to_vec()).unwrap().trim().to_string());
        cursor = cursor + 5;

        // Encryption
        file_header.insert("ENCRYP".to_string(),String::from_utf8(nitf[cursor..cursor+1].to_vec()).unwrap().trim().to_string());
        cursor = cursor + 1;

        // File Background Color
        file_header.insert("FBKGC".to_string(),format!("0x{:02X}{:02X}{:02X}",nitf[cursor],nitf[cursor+1],nitf[cursor+2]));
        cursor = cursor + 3;

        // Originator's Name
        if !String::from_utf8(nitf[cursor..cursor+24].to_vec()).unwrap().trim().to_string().is_empty() {
            file_header.insert("ONAME".to_string(),String::from_utf8(nitf[cursor..cursor+24].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 24;

        // Originator's Phone
        if !String::from_utf8(nitf[cursor..cursor+18].to_vec()).unwrap().trim().to_string().is_empty() {
            file_header.insert("OPHONE".to_string(),String::from_utf8(nitf[cursor..cursor+18].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 18;

        // File Length
        file_header.insert("FL".to_string(),String::from_utf8(nitf[cursor..cursor+12].to_vec()).unwrap());
        cursor = cursor + 12;

        // NITF File Header Length
        file_header.insert("HL".to_string(),String::from_utf8(nitf[cursor..cursor+6].to_vec()).unwrap());
        cursor = cursor + 6;

        // Number of Image Segments
        file_header.insert("NUMI".to_string(),String::from_utf8(nitf[cursor..cursor+3].to_vec()).unwrap());
        let mut num_of_image_seg = 0;
        for (index, value) in nitf[cursor..cursor+3].to_vec().iter().rev().enumerate() {
            num_of_image_seg += (*value as i32-48)*10_i32.pow(index as u32);
        }
        cursor = cursor + 3;

        for n in 1..=num_of_image_seg{
            // Length of nth Image Subheader
            file_header.insert(format!("LISH{:03}",n),String::from_utf8(nitf[cursor..cursor+6].to_vec()).unwrap());
            // Length of nth Image Segment
            file_header.insert(format!("LI{:03}",n),String::from_utf8(nitf[cursor+6..cursor+16].to_vec()).unwrap());
            cursor = cursor + 16;
        }

        // Number of Graphic Segments
        file_header.insert("NUMS".to_string(),String::from_utf8(nitf[cursor..cursor+3].to_vec()).unwrap());
        let mut num_of_graphic_seg = 0;
        for (index, value) in nitf[cursor..cursor+3].to_vec().iter().rev().enumerate() {
            num_of_graphic_seg += (*value as i32-48)*10_i32.pow(index as u32);
        }
        cursor = cursor + 3;

        for n in 1..=num_of_graphic_seg{
            // Length of nth Graphic Subheader
            file_header.insert(format!("LSSH{:03}",n),String::from_utf8(nitf[cursor..cursor+4].to_vec()).unwrap());
            // Length of nth Graphic Segment
            file_header.insert(format!("LS{:03}",n),String::from_utf8(nitf[cursor+4..cursor+10].to_vec()).unwrap());
            cursor = cursor + 10;
        }

        // Reserved for Future Use
        file_header.insert("NUMX".to_string(),String::from_utf8(nitf[cursor..cursor+3].to_vec()).unwrap());
        cursor = cursor + 3;

        // Number of Text Segments
        file_header.insert("NUMT".to_string(),String::from_utf8(nitf[cursor..cursor+3].to_vec()).unwrap());
        let mut num_of_text_seg = 0;
        for (index, value) in nitf[cursor..cursor+3].to_vec().iter().rev().enumerate() {
            num_of_text_seg += (*value as i32-48)*10_i32.pow(index as u32);
        }
        cursor = cursor + 3;

        for n in 1..=num_of_text_seg{
            // Length of nth Text Subheader
            file_header.insert(format!("LTSH{:03}",n),String::from_utf8(nitf[cursor..cursor+4].to_vec()).unwrap());
            // Length of nth Text Segment
            file_header.insert(format!("LT{:03}",n),String::from_utf8(nitf[cursor+4..cursor+9].to_vec()).unwrap());
            cursor = cursor + 9;
        }

        // Number of Data Extension Segments
        file_header.insert("NUMDES".to_string(),String::from_utf8(nitf[cursor..cursor+3].to_vec()).unwrap());
        let mut num_of_data_ext_seg = 0;
        for (index, value) in nitf[cursor..cursor+3].to_vec().iter().rev().enumerate() {
            num_of_data_ext_seg += (*value as i32-48)*10_i32.pow(index as u32);
        }
        cursor = cursor + 3;

        for n in 1..=num_of_data_ext_seg{
            // Length of nth Data Extension Segment Subheader
            file_header.insert(format!("LDSH{:03}",n),String::from_utf8(nitf[cursor..cursor+4].to_vec()).unwrap());
            // Length of nth Data Extension Segment
            file_header.insert(format!("LD{:03}",n),String::from_utf8(nitf[cursor+4..cursor+13].to_vec()).unwrap());
            cursor = cursor + 13;
        }

        // Number of Reserved Extension Segments
        file_header.insert("NUMRES".to_string(),String::from_utf8(nitf[cursor..cursor+3].to_vec()).unwrap());
        let mut num_of_reserved_ext_seg = 0;
        for (index, value) in nitf[cursor..cursor+3].to_vec().iter().rev().enumerate() {
            num_of_reserved_ext_seg += (*value as i32-48)*10_i32.pow(index as u32);
        }
        cursor = cursor + 3;

        for n in 1..=num_of_reserved_ext_seg{
            // Length of nth Reserved Extension Segment Subheader
            file_header.insert(format!("LRESH{:03}",n),String::from_utf8(nitf[cursor..cursor+4].to_vec()).unwrap());
            // Length of nth Reserved Extension Segment
            file_header.insert(format!("LRE{:03}",n),String::from_utf8(nitf[cursor+4..cursor+11].to_vec()).unwrap());
            cursor = cursor + 11;
        }

        // User Defined Header Data Length
        file_header.insert("UDHDL".to_string(),String::from_utf8(nitf[cursor..cursor+5].to_vec()).unwrap());
        let mut user_defined_header_data_length = 0;
        for (index, value) in nitf[cursor..cursor+5].to_vec().iter().rev().enumerate() {
            user_defined_header_data_length += (*value as i32-48)*10_i32.pow(index as u32);
        }
        cursor = cursor + 5;

        if user_defined_header_data_length > 0 {
            // User Defined Header Overflow Length
            file_header.insert("UDHOFL".to_string(),String::from_utf8(nitf[cursor..cursor+3].to_vec()).unwrap());
            cursor = cursor + 3;

            let mut i: usize = 0;
            while i < user_defined_header_data_length as usize {
                let tag = String::from_utf8(nitf[cursor+i..cursor+i+6].to_vec()).unwrap();
                i += 6;
                let mut length = 0;
                for (index, value) in nitf[cursor+i..cursor+i+5].to_vec().iter().rev().enumerate() {
                    length += (*value as i32-48)*10_i32.pow(index as u32);
                }
                i += 5;
                // User-Defined
                file_header.insert(tag,String::from_utf8(nitf[cursor+i..cursor+i+length as usize].to_vec()).unwrap().trim().to_string());
                i += length as usize;
            }
            cursor = cursor + i;
        }

        // Extended Header Data Length
        file_header.insert("XHDL".to_string(),String::from_utf8(nitf[cursor..cursor+5].to_vec()).unwrap());
        let mut extended_header_data_length = 0;
        for (index, value) in nitf[cursor..cursor+5].to_vec().iter().rev().enumerate() {
            extended_header_data_length += (*value as i32-48)*10_i32.pow(index as u32);
        }
        cursor = cursor + 5;

        if extended_header_data_length > 0 {
            // Extended Header Overflow Length
            file_header.insert("XHOFL".to_string(),String::from_utf8(nitf[cursor..cursor+3].to_vec()).unwrap().trim().to_string());
            cursor = cursor + 3;

            let mut i: usize = 0;
            while i < extended_header_data_length as usize - 3 {
                let tag = String::from_utf8(nitf[cursor+i..cursor+i+6].to_vec()).unwrap();
                i += 6;
                let mut length = 0;
                for (index, value) in nitf[cursor+i..cursor+i+5].to_vec().iter().rev().enumerate() {
                    length += (*value as i32-48)*10_i32.pow(index as u32);
                }
                i += 5;
                // Extended
                file_header.insert(tag,String::from_utf8(nitf[cursor+i..cursor+i+length as usize].to_vec()).unwrap().trim().to_string());
                i += length as usize;
            }
        }

        Ok(file_header)
    }



    fn parse_image_subheader(nitf: &Vec<u8>, offset: usize) -> std::io::Result<BTreeMap<String,String>> {

        let mut image_subheader = BTreeMap::new();

        let mut cursor = offset;

        // File Part Type
        image_subheader.insert("IM".to_string(),String::from_utf8(nitf[cursor..cursor+2].to_vec()).unwrap());
        cursor = cursor + 2;

        // Image Identifier 1
        image_subheader.insert("IID1".to_string(),String::from_utf8(nitf[cursor..cursor+10].to_vec()).unwrap());
        cursor = cursor + 10;

        // Image Data and Time
        image_subheader.insert("IDATIM".to_string(),
            // Year
            String::from_utf8(nitf[cursor..cursor+4].to_vec()).unwrap() + "/" +
            // Month
            &String::from_utf8(nitf[cursor+4..cursor+6].to_vec()).unwrap() + "/" +
            // Day
            &String::from_utf8(nitf[cursor+6..cursor+8].to_vec()).unwrap() + " " +
            // Hour
            &String::from_utf8(nitf[cursor+8..cursor+10].to_vec()).unwrap() + ":" +
            // Minute
            &String::from_utf8(nitf[cursor+10..cursor+12].to_vec()).unwrap() + ":" +
            // Second
            &String::from_utf8(nitf[cursor+12..cursor+14].to_vec()).unwrap()
        );
        cursor = cursor + 14;

        // Target Identifier
        if !String::from_utf8(nitf[cursor..cursor+17].to_vec()).unwrap().trim().to_string().is_empty() {
            image_subheader.insert("TGTID".to_string(),String::from_utf8(nitf[cursor..cursor+17].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 17;

        // Image Identifier 2
        if !String::from_utf8(nitf[cursor..cursor+80].to_vec()).unwrap().trim().to_string().is_empty() {
            image_subheader.insert("IID2".to_string(),String::from_utf8(nitf[cursor..cursor+80].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 80;

        // Image Security Classification
        image_subheader.insert("ISCLAS".to_string(),String::from_utf8(nitf[cursor..cursor+1].to_vec()).unwrap());
        cursor = cursor + 1;

        // Image Security Classifcation System
        if !String::from_utf8(nitf[cursor..cursor+2].to_vec()).unwrap().trim().to_string().is_empty() {
            image_subheader.insert("ISCLSY".to_string(),String::from_utf8(nitf[cursor..cursor+2].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 2;

        // Image Codewords
        if !String::from_utf8(nitf[cursor..cursor+11].to_vec()).unwrap().trim().to_string().is_empty() {
            image_subheader.insert("ISCODE".to_string(),String::from_utf8(nitf[cursor..cursor+11].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 11;

        // Image Control and Handling
        if !String::from_utf8(nitf[cursor..cursor+2].to_vec()).unwrap().trim().to_string().is_empty() {
            image_subheader.insert("ISCTLH".to_string(),String::from_utf8(nitf[cursor..cursor+2].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 2;

        // Image Releasing Instructions
        if !String::from_utf8(nitf[cursor..cursor+20].to_vec()).unwrap().trim().to_string().is_empty() {
            image_subheader.insert("ISREL".to_string(),String::from_utf8(nitf[cursor..cursor+20].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 20;

        // Image Declassification Type
        if !String::from_utf8(nitf[cursor..cursor+2].to_vec()).unwrap().trim().to_string().is_empty() {
            image_subheader.insert("ISDCTP".to_string(),String::from_utf8(nitf[cursor..cursor+2].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 2;

        // Image Declassification Date
        if !String::from_utf8(nitf[cursor..cursor+8].to_vec()).unwrap().trim().to_string().is_empty() {
            image_subheader.insert("ISDCDT".to_string(),
                // Year
                (String::from_utf8(nitf[cursor..cursor+4].to_vec()).unwrap() + "/" +
                // Month
                &String::from_utf8(nitf[cursor+4..cursor+6].to_vec()).unwrap() + "/" +
                // Day
                &String::from_utf8(nitf[cursor+6..cursor+8].to_vec()).unwrap()).trim().to_string()

            );
        }
        cursor = cursor + 8;

        // Image Declassification Excemption
        if !String::from_utf8(nitf[cursor..cursor+4].to_vec()).unwrap().trim().to_string().is_empty() {
            image_subheader.insert("ISDCXM".to_string(),String::from_utf8(nitf[cursor..cursor+4].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 4;

        // Image Downgrade
        if !String::from_utf8(nitf[cursor..cursor+1].to_vec()).unwrap().trim().to_string().is_empty() {
            image_subheader.insert("ISDG".to_string(),String::from_utf8(nitf[cursor..cursor+1].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 1;

        // Image Downgrade Date
        if !String::from_utf8(nitf[cursor..cursor+8].to_vec()).unwrap().trim().to_string().is_empty() {
            image_subheader.insert("ISDGDT".to_string(),
                // Year
                (String::from_utf8(nitf[cursor..cursor+4].to_vec()).unwrap() + "/" +
                // Month
                &String::from_utf8(nitf[cursor+4..cursor+6].to_vec()).unwrap() + "/" +
                // Day
                &String::from_utf8(nitf[cursor+6..cursor+8].to_vec()).unwrap()).trim().to_string()

            );
        }
        cursor = cursor + 8;

        // Image Classification Text
        if !String::from_utf8(nitf[cursor..cursor+43].to_vec()).unwrap().trim().to_string().is_empty() {
            image_subheader.insert("ISCLTX".to_string(),String::from_utf8(nitf[cursor..cursor+43].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 43;

        // Image Classification Authority Type
        if !String::from_utf8(nitf[cursor..cursor+1].to_vec()).unwrap().trim().to_string().is_empty() {
            image_subheader.insert("ISCATP".to_string(),String::from_utf8(nitf[cursor..cursor+1].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 1;

        // Image Classification Authority
        if !String::from_utf8(nitf[cursor..cursor+40].to_vec()).unwrap().trim().to_string().is_empty() {
            image_subheader.insert("ISCAUT".to_string(),String::from_utf8(nitf[cursor..cursor+40].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 40;

        // Image Classification Reason
        if !String::from_utf8(nitf[cursor..cursor+1].to_vec()).unwrap().trim().to_string().is_empty() {
            image_subheader.insert("ISCRSN".to_string(),String::from_utf8(nitf[cursor..cursor+1].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 1;

        // Image Security Source Date
        if !String::from_utf8(nitf[cursor..cursor+8].to_vec()).unwrap().trim().to_string().is_empty() {
            image_subheader.insert("ISSRDT".to_string(),
                // Year
                (String::from_utf8(nitf[cursor..cursor+4].to_vec()).unwrap() + "/" +
                // Month
                &String::from_utf8(nitf[cursor+4..cursor+6].to_vec()).unwrap() + "/" +
                // Day
                &String::from_utf8(nitf[cursor+6..cursor+8].to_vec()).unwrap()).trim().to_string()

            );
        }
        cursor = cursor + 8;

        // Image Classification Reason
        if !String::from_utf8(nitf[cursor..cursor+15].to_vec()).unwrap().trim().to_string().is_empty() {
            image_subheader.insert("ISCTLN".to_string(),String::from_utf8(nitf[cursor..cursor+15].to_vec()).unwrap().trim().to_string());
        }
        cursor = cursor + 15;

        Ok(image_subheader)
    }



    fn parse_graphic_subheader(nitf: &Vec<u8>, offset: usize) -> std::io::Result<BTreeMap<String,String>> {

        let mut graphic_subheader = BTreeMap::new();

        let mut cursor = offset;

        // File Part Type
        graphic_subheader.insert("SY".to_string(),String::from_utf8(nitf[cursor..cursor+2].to_vec()).unwrap());
        cursor = cursor + 2;

        // Graphic Identifier
        graphic_subheader.insert("SID".to_string(),String::from_utf8(nitf[cursor..cursor+10].to_vec()).unwrap());
        cursor = cursor + 10;

        Ok(graphic_subheader)
    }



    fn parse_text_subheader(nitf: &Vec<u8>, offset: usize) -> std::io::Result<BTreeMap<String,String>> {

        let mut text_subheader = BTreeMap::new();

        let mut cursor = offset;

        // File Part Type
        text_subheader.insert("TE".to_string(),String::from_utf8(nitf[cursor..cursor+2].to_vec()).unwrap());
        cursor = cursor + 2;

        // Graphic Identifier
        text_subheader.insert("TEXTID".to_string(),String::from_utf8(nitf[cursor..cursor+7].to_vec()).unwrap());
        cursor = cursor + 7;

        Ok(text_subheader)
    }



    fn parse_data_ext_seg_subheader(nitf: &Vec<u8>, offset: usize) -> std::io::Result<BTreeMap<String,String>> {

        let mut data_ext_seg_subheader = BTreeMap::new();

        let mut cursor = offset;

        // File Part Type
        data_ext_seg_subheader.insert("DE".to_string(),String::from_utf8(nitf[cursor..cursor+2].to_vec()).unwrap());
        cursor = cursor + 2;

        // Graphic Identifier
        data_ext_seg_subheader.insert("DESID".to_string(),String::from_utf8(nitf[cursor..cursor+25].to_vec()).unwrap());
        cursor = cursor + 25;

        Ok(data_ext_seg_subheader)
    }
}
