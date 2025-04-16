/*!
Copyright 2025 Lucas Walter

Connect to a ntrip server.
Receive nmea messages on a ros topic and forward those to the ntrip server.
Receive rtcm messages from the ntrip server and publish those on a ros topic.
*/

use anyhow::Context;
use clap::command;
use ntrip_client::ntrip_client::{NtripClientError, NtripConfig};
use roslibrust::ros1::NodeHandle;
use roslibrust_util::{nmea_msgs, mavros_msgs::RTCM};
use rtcm_parser::rtcm_parser::RtcmParser;
use std::collections::HashMap;
use std::time::Duration;
use tf_roslibrust::tf_util;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::time::timeout;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()?;

    let (nh, _full_node_name, params, remaps) = {
        let mut params = HashMap::from([
            ("_name".to_string(), "ntrip_client".to_string()),
            ("host".to_string(), "".to_string()),
            ("port".to_string(), "".to_string()),
            ("mountpoint".to_string(), "".to_string()),
            ("username".to_string(), "".to_string()),
            ("password".to_string(), "".to_string()),
        ]);
        let mut remaps = HashMap::from([
            ("nmea".to_string(), "nmea".to_string()),
            ("rtcm".to_string(), "rtcm".to_string()),
        ]);

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
        let rtcm_pub = nh
            .advertise::<RTCM>(rtcm_topic, 3, false)
            .await?;

        let nmea_topic = remaps.get("nmea").context("no nmea topic found")?;
        let mut nmea_sub = nh
            .subscribe::<nmea_msgs::Sentence>(nmea_topic, 2)
            .await?;

        let host = params.get("host").unwrap();
        let port = params.get("port").unwrap();
        let mountpoint = params.get("mountpoint").unwrap();
        let username = params.get("username").unwrap();
        let password = params.get("password").unwrap();
        let server = NtripConfig::new(host, port, mountpoint, username, password);

        log::info!("Connecting to server with config: {server:?}");
        // TODO(lucasw) need the error type to be able to use '?' here
        let connection = server.connect().await.unwrap();
        log::info!("connected: {connection:?}");

        let mut stream = ntrip_client::ntrip_client::init_stream(connection).await.unwrap();

        let (nmea_sender, mut nmea_receiver) = mpsc::channel(10);

        {
            tokio::spawn(async move {
                while let Some(nmea) = nmea_sub.next().await {
                    match nmea {
                        Ok(nmea) => {
                            nmea_sender.send(nmea).await.unwrap();
                        }
                        Err(rv) => {
                            log::error!("error with nmea reception: {rv:?}");
                        }
                    }
                }

                panic!("done with nmea reception, need to restart");
            });
        }

        // TODO(lucasw) need an NtripClient to own this
        let mut parser = RtcmParser::new();

        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(10));
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    log::warn!("ctrl-c, exiting");
                    // TODO(lucasw) need to bring down the egui app too
                    break;
                }
                _ = interval.tick() => {
                    // these details belong in NtripClient impl
                    let mut buffer = [0; 256];

                    // TODO(lucasw) look at failure of timeout or read() instead of just '??'
                    let n = timeout(Duration::from_secs(5), stream.read(&mut buffer)).await??;
                    if n == 0 {
                        continue;
                    }

                    let messages = parser.parse(&buffer[..n]);
                    for message in messages {
                        let mut rtcm = RTCM::default();
                        rtcm.header.stamp = tf_util::stamp_now();
                        rtcm.header.frame_id = "odom".to_string();
                        rtcm.data = message;
                        rtcm_pub.publish(&rtcm).await?;
                    }
                }
                Some(nmea) = nmea_receiver.recv() => {
                    stream.write_all(nmea.sentence.as_bytes()).await?;
                }
            }
        }
    }

    Ok(())
}
