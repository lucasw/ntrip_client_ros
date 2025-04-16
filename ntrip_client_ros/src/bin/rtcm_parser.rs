/*!
Copyright 2025 Lucas Walter

Connect to a ntrip server.
Receive nmea messages on a ros topic and forward those to the ntrip server.
Receive rtcm messages from the ntrip server and publish those on a ros topic.
*/

use anyhow::Context;
use clap::command;
use roslibrust::ros1::NodeHandle;
use roslibrust_util::mavros_msgs;
use rtcm_parser::rtcm_parser::Rtcm;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()?;

    let (nh, _full_node_name, _params, remaps) = {
        let mut params = HashMap::from([("_name".to_string(), "rtcm_parser".to_string())]);
        let mut remaps = HashMap::from([("rtcm".to_string(), "rtcm".to_string())]);

        let (_ns, full_node_name, remaining_args) =
            roslibrust_util::get_params_remaps(&mut params, &mut remaps);

        // using clap only for version reporting currently
        let _matches = command!().get_matches_from(remaining_args);

        let ros_master_uri =
            std::env::var("ROS_MASTER_URI").unwrap_or("http://localhost:11311".to_string());
        let nh = NodeHandle::new(&ros_master_uri, &full_node_name).await?;
        log::info!("{full_node_name} connected to roscore at {ros_master_uri}");

        (nh, full_node_name, params, remaps)
    };

    {
        let rtcm_topic = remaps.get("rtcm").context("no rtcm topic found")?;
        let mut rtcm_sub = nh.subscribe::<mavros_msgs::RTCM>(rtcm_topic, 4).await?;

        {
            tokio::spawn(async move {
                while let Some(rtcm) = rtcm_sub.next().await {
                    match rtcm {
                        Ok(rtcm) => {
                            if rtcm.data.len() <= 6 {
                                continue;
                            }
                            let rtcm = Rtcm::parse(&rtcm.data[3..rtcm.data.len() - 3]).unwrap();
                            match rtcm {
                                Rtcm::Rtcm1005(msg) => {
                                    println!("{msg:?}");
                                }
                                Rtcm::Rtcm1006(msg) => {
                                    println!("{msg:?}");
                                }
                                Rtcm::Rtcm1019(msg) => {
                                    println!("{msg:?}");
                                }
                                Rtcm::RtcmMSM7(msg) => {
                                    println!("{msg:?}");
                                }
                                _ => {
                                    println!("unknown: {rtcm:?}");
                                }
                            }
                        }
                        Err(rv) => {
                            log::error!("error with rtcm reception: {rv:?}");
                        }
                    }
                }

                panic!("done with rtcm reception, exiting");
            });
        }

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    log::warn!("ctrl-c, exiting");
                    // TODO(lucasw) need to bring down the egui app too
                    break;
                }
            }
        }
    }

    Ok(())
}
