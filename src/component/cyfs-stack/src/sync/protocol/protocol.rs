
/*

/diff
req:
category: root_state
path,  // target object in objectmap's tree path
?current // current value 

resp:
result,
target, // full objectmap or diff objectmap
objects, // some objects in body, in select response format


/object_info
req:
begin_seq,
count,

resp
list: [{
    object_id,
    seq,
    update_time,
}]


/objects
req: 
object_list: [{
    object_id,
}]

resp: 
[select_resp]
*/


