//----------
//Vue

var cmds_joinsign = "🀡🀡";
var files_joinsign = "🀅🀅";

var vue = new Vue({
    el: '#app-main',

    data: {
        message: 'Hello Vue.js!',
        wvmessage: null,
        filelist: null,
        filelistlen: 0,
        fileliststring: null,
        userdata: 0,
        width: 800,
        height: 800,
        maxsize: 800,
        percentage: 50,
        format: "jpg",
        quality: 90,
        mode: "ByWidth",
        includesubfolder: true,
        donotenlager: true,
        donotenlager_checkchange_org: true,
        sourcefolder: null,
        exportfolder: null,
        autoexpotfolder: true,
        process_n:0,
        process_total:0,
        process_p:0,
        process_visable: false,
        process_ing: false,
        isProcessDisabled: false,
        isdone: null,
        isButtonDisabled: false,
        isResizeDisabled: true,
        flashmsg: null,
        failures:0,
    },

    methods: {
        showmessage(msg) {
            if(this.wvmessage == null){
                this.wvmessage = msg;
            }else{
                this.wvmessage += "\n" + msg;
            };
        },

        flashmessage(msg){
            if (msg == "null"){
                this.flashmsg = null;
            } else {
                this.flashmsg = msg;
            }
        },

        ifnull0(v) {
            if(v == null){v = 0};
        },

        resize_sender() {
            if(this.autoexpotfolder == true){
                this.exportfolder = [this.sourcefolder,"resized"].join("_");
            };
            this.ifnull0(this.width);
            this.ifnull0(this.height);
            this.ifnull0(this.maxsize);
            this.ifnull0(this.percentage);
            if(this.quality == null){this.quality = 90};
            
            var msgv = [
            "resize",
            this.fileliststring,
            this.sourcefolder,
            this.exportfolder,
            this.format,
            this.mode,
            this.width,
            this.height,
            this.maxsize,
            this.percentage,
            this.quality,
            this.donotenlager,
            ]

            var msg = msgv.join(cmds_joinsign);
            external.invoke(msg);

            this.wvmessage = "Export to : " + this.exportfolder;
            $('#progress').progress('reset');
            $('#progress').progress('set total', this.filelistlen,);
            this.process_n = 0;
            this.isdone = null;
            this.isButtonDisabled = true;
            this.isResizeDisabled = true;
        },

        resize_process(n){
            this.check_process();
            this.process_n = n;
            // %
            var p = n / this.filelistlen * 100;
            this.process_p = p.toFixed(1);
            // if processing, lock the buttons
            this.process_ing = true;

            $('#progress').progress(
                'set progress', this.process_n,
            );

            if(this.process_n >= this.filelistlen){
               doisdone();
            }
        },

        check_process(){
            // check if with fails count is not 0
            if ((this.isdone != null) && (this.failures > 0)) {
                $("#progress").addClass("inverted");
                $("#progress").addClass("error");
                $("#progress").removeClass("indicating");
                $("#progress").addClass("disabled");
                this.isProcessDisabled = true;
            };
            // at begin time
            if (this.isdone == null) {
                this.isProcessDisabled = false;
                $("#progress").removeClass("disabled");
                $("#progress").removeClass("inverted");
                $("#progress").removeClass("error");
                $("#progress").addClass("indicating");
            };
        },

        doisdone(){
            // resize done, release buttons
            this.isdone = "Finish";
            this.process_ing = false;
            this.isButtonDisabled = false;
            this.isResizeDisabled = false;
            this.check_process();
        },

        checkmode() {
            if(this.mode == 'Percentage'){
                this.donotenlager_checkchange_org = this.donotenlager;
                this.donotenlager = false;
                $("div#donotenlager_div").addClass("disabled");
                $("input#donotenlager").attr("disabled","disabled");
            }else{
                this.donotenlager = this.donotenlager_checkchange_org;
                $("div#donotenlager_div").removeClass("disabled");
                $("input#donotenlager").removeAttr("disabled");

            }
        },

        checkdonotenlager(){
            this.donotenlager_checkchange_org = this.donotenlager;
        },

        checksubfolder(){
            var arg2 = "noSubfolder";
            if(this.includesubfolder){arg2 = 'includeSubfolder';}
            var arg3 = this.sourcefolder;
            var msgv = ['checkFolder', arg2, arg3];
            var msg = msgv.join(cmds_joinsign);
            external.invoke(msg);
        },

        checkResizeDisabled(){
            if ((this.fileliststring == null) && (this.exportfolder == null)){
                this.isResizeDisabled = true;
            };
            if ((this.sourcefolder == null) && this.autoexpotfolder){
              this.isResizeDisabled = true;  
          };
        },

        checkExportFolder(){
            if(this.autoexpotfolder == true){
                this.exportfolder = null;
            }
        },

        openFolder() {
            var msg = 'openFolder';
            if(this.includesubfolder){
                msg = msg + cmds_joinsign + 'includeSubfolder';
            }else{
                msg = msg + cmds_joinsign + 'NoSubfolder';
            }
            external.invoke(msg);
        },

        openMulti() {
            var msg = 'openMulti';
            external.invoke(msg);
        },

        selectExportFolder() {
            external.invoke('selectExportFolder');
        },

        downtextarea(){
            this.wvmessage($("#infotextarea").scrollHeight);
            this.wvmessage($("#infotextarea").scrollTop);
            $("#infotextarea").scrollTop = $("#infotextarea").scrollHeight;
        },
    }
})

//-------------
//Semantic-ui
$('.ui.dropdown').dropdown();
$('#progress').progress({label:"ratio"});

// textarea always down
function scrollToBottom() {
  $('#infotextarea').scrollTop($('#infotextarea')[0].scrollHeight);
}

