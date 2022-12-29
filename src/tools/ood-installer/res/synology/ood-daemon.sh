#!/bin/sh

### BEGIN INIT INFO
# Provides:          OOD
# Required-Start:    $remote_fs $syslog
# Required-Stop:     $remote_fs $syslog
# Should-Start:      $network $time
# Should-Stop:       $network $time
# Default-Start:     2 3 4 5
# Default-Stop:      0 1 6
### END INIT INFO

PID_FILE=/var/run/ood-daemon.pid

case "$1" in 
start)
   /cyfs/services/ood-daemon/current/bin/ood-daemon --start --startup-mode >/dev/null 2>&1 &
   pid="$!"
   exit_code="$?"
   echo $pid>$PID_FILE
   echo "ood-daemon start pid:[$pid] exit_code:[$exit_code]"
   ;;
stop)
   /cyfs/services/ood-daemon/current/bin/ood-daemon --stop
   exit_code="$?"
   if test -f "$PID_FILE"; then
      rm $PID_FILE
   fi
   
   if [ $exit_code -eq 0 ]; then
      echo "ood-daemon service not running"
      exit 1
    elif [ $exit_code -gt 0 ]; then
       echo "ood-daemon service stopped [$exit_code]" 
    else
       echo "ood-daemon service stoped failed!" 
    fi
   ;;
restart)
   $0 stop
   $0 start
   ;;
status)
   /cyfs/services/ood-daemon/current/bin/ood-daemon --status
   echo "check status exit: $?"
   if [ "$?" -eq 0 ]; then
      echo "ood-daemon service not running"
      exit 1
    elif [ "$?" -gt 0 ]; then
       echo "ood-daemon service is running" 
    else
       echo "ood-daemon service is running but old" 
    fi
   ;;
*)
   echo "Usage: $0 {start|stop|status|restart}"
esac

exit 0 