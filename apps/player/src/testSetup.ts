// jsdom 目前只实现了 <dialog> 的 open 属性。
HTMLDialogElement.prototype.showModal ??= function () {
    this.open = true;
};
HTMLDialogElement.prototype.close ??= function () {
    this.open = false;
};
