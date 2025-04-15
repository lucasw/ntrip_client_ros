# ntrip_client_ros
ntrip gps corrections client using rust and roslibrust


Using local copies of subsets of nmea_msgs and mavros_msgs:

```
ROS_PACKAGE_PATH=`pwd`/msgs:`rospack find actionlib_msgs`:`rospack find geometry_msgs`:`rospack find tf2_msgs`:`rospack find std_msgs`:`rospack find sensor_msgs` cargo build --release
```
